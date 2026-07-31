use super::*;
use crate::models::driver::Driver;
use crate::models::team::placeholder_team_from_db;
use crate::simulation::context::SimDriver;

fn piloto(id: &str, skill: f64) -> SimDriver {
    let mut d = Driver::create_player(id.to_string(), format!("P{id}"), "BR".to_string(), 26);
    d.is_jogador = false;
    d.atributos.skill = skill;
    d.atributos.ritmo_classificacao = skill;
    d.atributos.adaptabilidade = 60.0;
    d.atributos.experiencia = 60.0;
    d.atributos.consistencia = 65.0;
    d.atributos.aggression = 50.0;
    d.atributos.smoothness = 50.0;
    d.atributos.confianca = 60.0;
    d.motivacao = 70.0;
    d.corridas_na_categoria = 40;
    let team = placeholder_team_from_db(
        format!("T{id}"),
        format!("E{id}"),
        "gt4".to_string(),
        "2026-01-01T00:00:00".to_string(),
    );
    SimDriver::from_driver_and_team(&d, &team)
}

/// Contexto neutro com pista conhecida — isola um elo por vez.
fn ctx_pista_dominada() -> ContextoDoPiloto {
    ContextoDoPiloto {
        conhecimento_de_pista: TrackKnowledge {
            starts: 5,
            best_finish: Some(1),
            last_season: Some(3),
        },
        comprimento_da_pista_km: 4.0,
        ..Default::default()
    }
}

// ─────────── (a)–(d): o contrato do harness ───────────

#[test]
fn contrato_devolve_o_estado_de_forma_em_vez_de_gravar() {
    // (d) A esteira não escreve em banco nenhum: recebe a forma e devolve a avançada. É isso
    // que tira o `if persistence_mode` do meio da simulação.
    let grid = vec![piloto("01", 70.0), piloto("02", 68.0)];
    let ctxs = vec![ctx_pista_dominada(), ctx_pista_dominada()];
    let antes = vec![0.0, 0.0];

    let r = aplicar_esteira(&grid, &ctxs, 1, 1, 523, &antes, &Default::default());
    assert_eq!(r.estado_de_forma.len(), 2);
    assert_eq!(r.grid.len(), 2);
    assert_eq!(r.deltas.len(), 2);
    assert!(
        r.estado_de_forma.iter().any(|f| *f != 0.0),
        "a forma tinha que ter avançado"
    );

    // E o AR(1) é sequência: alimentar de volta o estado devolvido continua a série.
    let r2 = aplicar_esteira(
        &grid,
        &ctxs,
        1,
        2,
        523,
        &r.estado_de_forma,
        &Default::default(),
    );
    assert_ne!(r2.estado_de_forma, r.estado_de_forma);
}

#[test]
fn contrato_e_puro_e_reprodutivel() {
    let grid = vec![piloto("01", 70.0)];
    let ctxs = vec![ctx_pista_dominada()];
    let a = aplicar_esteira(&grid, &ctxs, 2, 5, 166, &[0.3], &Default::default());
    let b = aplicar_esteira(&grid, &ctxs, 2, 5, 166, &[0.3], &Default::default());
    assert_eq!(a.grid[0].skill, b.grid[0].skill);
    assert_eq!(a.grid[0].ritmo_classificacao, b.grid[0].ritmo_classificacao);
    assert_eq!(a.estado_de_forma, b.estado_de_forma);
}

#[test]
fn contrato_temporada_e_rodada_separam_afinidade_de_acerto() {
    // (c) A afinidade é indexada por (piloto, PISTA) e não muda com a rodada; o acerto é
    // indexado por (temporada, RODADA, equipe) e muda. É essa assinatura que permite ao
    // harness isolar as duas fixando a pista.
    let grid = vec![piloto("01", 70.0)];
    let ctxs = vec![ctx_pista_dominada()];

    let r1 = aplicar_esteira(&grid, &ctxs, 1, 1, 523, &[0.0], &Default::default());
    let r2 = aplicar_esteira(&grid, &ctxs, 1, 2, 523, &[0.0], &Default::default());
    let af = |r: &ResultadoDaEsteira| {
        r.deltas[0].pretendido_de(EloDaEsteira::AfinidadeDePista, Canal::Corrida)
    };
    let ac = |r: &ResultadoDaEsteira| {
        r.deltas[0].pretendido_de(EloDaEsteira::AcertoDeFimDeSemana, Canal::Corrida)
    };
    assert_eq!(af(&r1), af(&r2), "afinidade não pode variar com a rodada");
    assert_ne!(ac(&r1), ac(&r2), "acerto TEM que variar com a rodada");

    // E a afinidade muda com a pista.
    let r3 = aplicar_esteira(&grid, &ctxs, 1, 1, 166, &[0.0], &Default::default());
    assert_ne!(af(&r1), af(&r3), "afinidade tem que variar com a pista");
}

#[test]
fn contrato_grid_sem_contexto_usa_o_piloto_neutro() {
    // O harness monta grid sintético e não tem histórico de circuito, lesão nem classificação.
    let grid = vec![piloto("01", 70.0)];
    let r = aplicar_esteira(&grid, &[], 1, 1, 523, &[0.0], &Default::default());
    assert_eq!(r.grid.len(), 1);
    // Pista desconhecida (o default) cobra penalidade de conhecimento.
    assert!(
        r.deltas[0].pretendido_de(EloDaEsteira::ConhecimentoDePista, Canal::Corrida) < 0.0,
        "pista nova tem que custar"
    );
}

// ─────────── (e): o delta por elo × canal ───────────

#[test]
fn delta_traz_os_oito_elos_nos_dois_canais() {
    let grid = vec![piloto("01", 70.0)];
    let ctxs = vec![ctx_pista_dominada()];
    let r = aplicar_esteira(&grid, &ctxs, 1, 1, 523, &[0.0], &Default::default());
    let d = &r.deltas[0];

    for elo in [
        EloDaEsteira::ConhecimentoDePista,
        EloDaEsteira::AdaptacaoDeCategoria,
        EloDaEsteira::Lesao,
        EloDaEsteira::AfinidadeDePista,
        EloDaEsteira::FormaDoMomento,
        EloDaEsteira::AcertoDeFimDeSemana,
        EloDaEsteira::Motivacao,
        EloDaEsteira::Pressao,
    ] {
        for canal in [Canal::Corrida, Canal::Classificacao] {
            assert!(
                d.pretendido
                    .iter()
                    .any(|p| p.elo == elo && p.canal == canal),
                "faltou {elo:?} no canal {canal:?}"
            );
        }
    }
    assert_eq!(d.pretendido.len(), 16, "8 elos × 2 canais");
    assert_eq!(d.aplicado.len(), 12, "6 passos × 2 canais");
}

#[test]
fn as_tres_camadas_de_forma_vem_separadas() {
    // A fase 1 da campanha redistribui entre afinidade, forma e acerto ANTES de mexer na soma.
    // Um número agregado de "forma total" esconderia exatamente o que ela precisa decidir.
    let grid = vec![piloto("01", 70.0)];
    let ctxs = vec![ctx_pista_dominada()];
    let r = aplicar_esteira(&grid, &ctxs, 4, 7, 413, &[0.8], &Default::default());
    let d = &r.deltas[0];

    let af = d.pretendido_de(EloDaEsteira::AfinidadeDePista, Canal::Corrida);
    let fo = d.pretendido_de(EloDaEsteira::FormaDoMomento, Canal::Corrida);
    let ac = d.pretendido_de(EloDaEsteira::AcertoDeFimDeSemana, Canal::Corrida);
    assert!(af != 0.0 && fo != 0.0 && ac != 0.0, "{af} {fo} {ac}");

    // E a soma das três é o que o passo de arredondamento recebeu.
    let soma = af + fo + ac;
    let pedido_do_passo: f64 = d
        .pretendido
        .iter()
        .filter(|p| {
            p.elo.passo() == PassoDeArredondamento::CamadasDeForma && p.canal == Canal::Corrida
        })
        .map(|p| p.pontos)
        .sum();
    assert!((soma - pedido_do_passo).abs() < 1e-9);
}

#[test]
fn a_assimetria_de_canal_da_afinidade_fica_visivel() {
    // A afinidade recebe MULT_AFINIDADE_QUALI na classificação; as outras duas camadas não
    // recebem nada. Se o delta viesse colapsado num número por elo, essa assimetria ficaria
    // invisível justamente no parâmetro candidato a INVERTER de sinal, não só a diminuir.
    let grid = vec![piloto("01", 70.0)];
    let ctxs = vec![ctx_pista_dominada()];
    let r = aplicar_esteira(&grid, &ctxs, 1, 3, 523, &[0.0], &Default::default());
    let d = &r.deltas[0];

    let af_c = d.pretendido_de(EloDaEsteira::AfinidadeDePista, Canal::Corrida);
    let af_q = d.pretendido_de(EloDaEsteira::AfinidadeDePista, Canal::Classificacao);
    assert!(
        (af_q - af_c * crate::simulation::forma::MULT_AFINIDADE_QUALI).abs() < 1e-9,
        "a afinidade tem que entrar amplificada na quali: {af_c} → {af_q}"
    );

    // A FORMA é simétrica entre canais, e continua tendo que ser: ela é o estado do piloto, e
    // quem está em baixa está em baixa no sábado e no domingo.
    assert!(
        (d.pretendido_de(EloDaEsteira::FormaDoMomento, Canal::Corrida)
            - d.pretendido_de(EloDaEsteira::FormaDoMomento, Canal::Classificacao))
        .abs()
            < 1e-9,
        "a forma do momento não devia diferir entre canais — é estado do piloto, não do carro"
    );

    // O ACERTO, por outro lado, TEM que diferir. Este assert já foi o oposto dele: enquanto o
    // acerto era um número só entregue aos dois canais, o fim de semana andava em bloco e o pole
    // vencia 79% das corridas. Ver `forma::CORRELACAO_ENTRE_TRIMS`.
    let ac_c = d.pretendido_de(EloDaEsteira::AcertoDeFimDeSemana, Canal::Corrida);
    let ac_q = d.pretendido_de(EloDaEsteira::AcertoDeFimDeSemana, Canal::Classificacao);
    assert!(
        (ac_q - ac_c).abs() > 1e-9,
        "trim de quali e trim de corrida colapsaram no mesmo número ({ac_c}) — a corrida voltou \
         a ser decidida no sábado"
    );
}

#[test]
fn quantizacao_fica_mensuravel_por_passo() {
    // A pergunta que o item (e) existe para responder: "o MAX_PENALTY = 8,0 do conhecimento de
    // pista entregou 8 pontos?". Pista nova + adaptabilidade baixa = penalidade máxima.
    let mut sd = piloto("01", 70.0);
    sd.adaptabilidade = 0;
    let grid = vec![sd];
    let ctxs = vec![ContextoDoPiloto {
        comprimento_da_pista_km: 2.5,
        ..Default::default()
    }];
    let r = aplicar_esteira(&grid, &ctxs, 1, 1, 523, &[0.0], &Default::default());
    let d = &r.deltas[0];

    let pedido = d.pretendido_de(EloDaEsteira::ConhecimentoDePista, Canal::Corrida);
    let aplicado = d.aplicado_de(PassoDeArredondamento::ConhecimentoDePista, Canal::Corrida);
    assert!(
        (pedido + 8.0).abs() < 1e-9,
        "pista nova com adapt 0 tem que pedir os 8 pontos cheios, pediu {pedido}"
    );
    // Neste caso a penalidade é inteira, então nada se perde — e o teste mostra que a conta
    // FECHA, que é o ponto: a perda passa a ser um número em vez de uma suspeita.
    assert!(
        (aplicado - pedido).abs() < 0.5,
        "pedido {pedido} vs aplicado {aplicado}"
    );
    assert!(
        d.perda_por_quantizacao(PassoDeArredondamento::ConhecimentoDePista, Canal::Corrida)
            .abs()
            < 0.5
    );
}

#[test]
fn ajuste_pequeno_e_engolido_pelo_u8_e_isso_aparece_no_relatorio() {
    // O outro lado da mesma moeda: um pedido de fração de ponto pode sumir inteiro no
    // arredondamento. Antes deste relatório isso era invisível; agora é um número nomeado.
    let grid = vec![piloto("01", 70.0)];
    let ctxs = vec![ContextoDoPiloto {
        conhecimento_de_pista: TrackKnowledge {
            starts: 5,
            best_finish: Some(1),
            last_season: Some(3),
        },
        comprimento_da_pista_km: 4.0,
        // Lesão minúscula: 0,3% de 70 ≈ 0,21 ponto.
        fracao_de_lesao: Some(0.003),
        ..Default::default()
    }];
    let r = aplicar_esteira(&grid, &ctxs, 1, 1, 523, &[0.0], &Default::default());
    let d = &r.deltas[0];

    let pedido = d.pretendido_de(EloDaEsteira::Lesao, Canal::Corrida);
    let aplicado = d.aplicado_de(PassoDeArredondamento::Lesao, Canal::Corrida);
    assert!(pedido < 0.0 && pedido > -0.5, "pedido pequeno: {pedido}");
    assert_eq!(aplicado, 0.0, "o u8 engoliu o ajuste inteiro");
    assert!(
        d.perda_por_quantizacao(PassoDeArredondamento::Lesao, Canal::Corrida)
            .abs()
            > 0.1,
        "a perda tinha que aparecer nomeada"
    );
}

/// **A anulação determinística.** Roda com:
///
///   cargo test --manifest-path src-tauri/Cargo.toml anulacao_por_elo -- --nocapture
///
/// O mecanismo, e por que ele é pior que "ruído de arredondamento": a base é `u8`, logo um
/// inteiro EXATO, e a operação é `round(base + δ)`. Para qualquer `|δ| < 0,5` o resultado é
/// exatamente `base` — **sempre**, não às vezes. Um elo cuja magnitude típica fica abaixo de
/// meio ponto não perde resolução: ele é ANULADO, de forma determinística e total.
///
/// As três camadas de `forma` escapam porque somam em `f64` antes de arredondar uma vez só. Os
/// cinco elos restantes arredondam separados, e é neles que a anulação mora.
///
/// Este teste mede sobre o gerador REAL de pilotos (`Driver::generate_for_category`), não sobre
/// grid sintético, porque a pergunta é sobre magnitude típica e magnitude típica depende da
/// distribuição real de atributos.
#[test]
fn anulacao_por_elo_e_deterministica_e_mensuravel() {
    use crate::simulation::pressure::PressureEffect;
    use rand::{rngs::StdRng, SeedableRng};
    use std::collections::HashSet;

    /// Acumulador de um passo: quanto foi pedido e quantas vezes o `u8` engoliu tudo.
    #[derive(Default)]
    struct Acc {
        pedidos: Vec<f64>,
        anulados: usize,
        casos: usize,
    }

    let categorias = [("mazda_rookie", 0u8, "rookie"), ("gt3", 4u8, "topo")];
    println!("\n=== Anulação determinística por elo (base u8, |δ| < 0,5 ⇒ δ vira 0) ===");

    for (categoria, tier, rotulo) in categorias {
        let mut rng = StdRng::seed_from_u64(20260730);
        let mut nomes = HashSet::new();
        let pilotos =
            Driver::generate_for_category(categoria, tier, "medio", 24, &mut nomes, &mut rng);

        let mut por_passo: std::collections::HashMap<String, Acc> = Default::default();

        // As situações que um piloto de verdade atravessa numa temporada: pista nova até
        // dominada, estreante até veterano na categoria, são até machucado.
        for starts in [0u32, 1, 2, 3, 6] {
            for corridas_na_categoria in [0i32, 2, 5, 9, 40] {
                for fracao_de_lesao in [None, Some(0.02), Some(0.12)] {
                    let grid: Vec<SimDriver> = pilotos
                        .iter()
                        .map(|d| {
                            let team = placeholder_team_from_db(
                                format!("T{}", d.id),
                                "E".to_string(),
                                categoria.to_string(),
                                "2026-01-01T00:00:00".to_string(),
                            );
                            let mut sd = SimDriver::from_driver_and_team(d, &team);
                            sd.corridas_na_categoria = corridas_na_categoria;
                            sd
                        })
                        .collect();
                    let ctxs: Vec<ContextoDoPiloto> = grid
                        .iter()
                        .map(|_| ContextoDoPiloto {
                            conhecimento_de_pista: TrackKnowledge {
                                starts,
                                best_finish: None,
                                last_season: Some(3),
                            },
                            comprimento_da_pista_km: 4.0,
                            fracao_de_lesao,
                            // ATENÇÃO ao ler a linha "pressão" do relatório: este `0.3` é
                            // ESCOLHIDO, não medido — a pressão real depende da classificação
                            // do campeonato, que não existe aqui. A linha mede o que o
                            // `headroom_pace_mult` faz com um pedido de 0,3, e não a
                            // distribuição real de pedidos. As outras cinco linhas usam
                            // entrada real (gerador de pilotos + as próprias funções dos elos).
                            pressao: Some(EntradaDePressao {
                                campeonato: PressureEffect {
                                    pace_delta: 0.3,
                                    error_mult: 1.0,
                                },
                                ..Default::default()
                            }),
                        })
                        .collect();
                    let estado = vec![0.0; grid.len()];
                    let r = aplicar_esteira(&grid, &ctxs, 3, 5, 523, &estado, &Default::default());

                    for d in &r.deltas {
                        for (passo, nome) in [
                            (
                                PassoDeArredondamento::ConhecimentoDePista,
                                "conhecimento de pista",
                            ),
                            (
                                PassoDeArredondamento::AdaptacaoDeCategoria,
                                "adaptação de categoria",
                            ),
                            (PassoDeArredondamento::Lesao, "lesão"),
                            (
                                PassoDeArredondamento::CamadasDeForma,
                                "camadas de forma (3, somadas)",
                            ),
                            (PassoDeArredondamento::Motivacao, "motivação"),
                            (PassoDeArredondamento::Pressao, "pressão"),
                        ] {
                            let pedido: f64 = d
                                .pretendido
                                .iter()
                                .filter(|p| p.elo.passo() == passo && p.canal == Canal::Corrida)
                                .map(|p| p.pontos)
                                .sum();
                            if pedido == 0.0 {
                                continue;
                            }
                            let acc = por_passo.entry(nome.to_string()).or_default();
                            acc.pedidos.push(pedido.abs());
                            acc.casos += 1;
                            if d.aplicado_de(passo, Canal::Corrida) == 0.0 {
                                acc.anulados += 1;
                            }
                        }
                    }
                }
            }
        }

        println!("\n-- {rotulo} ({categoria}) --");
        let mut nomes_ordenados: Vec<&String> = por_passo.keys().collect();
        nomes_ordenados.sort();
        for nome in nomes_ordenados {
            let acc = &por_passo[nome];
            let mut p = acc.pedidos.clone();
            p.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let mediana = p[p.len() / 2];
            let abaixo_de_meio =
                p.iter().filter(|v| **v < 0.5).count() as f64 / p.len() as f64 * 100.0;
            println!(
                "{nome:<30} | |δ| mediano {mediana:5.2} | {abaixo_de_meio:5.1}% dos pedidos < 0,5 \
                 | ANULADO em {:5.1}% dos casos",
                acc.anulados as f64 / acc.casos as f64 * 100.0
            );
        }
    }

    // A propriedade em si, provada direto e sem depender de distribuição: sobre base inteira,
    // meio ponto é o divisor de águas exato.
    let mut base: u8 = 70;
    let inalterado = somar(&mut base, 0.49);
    assert_eq!(inalterado, 0.0, "0,49 sobre base inteira é anulado SEMPRE");
    assert_eq!(base, 70);
    let mut base2: u8 = 70;
    let aplicado = somar(&mut base2, 0.51);
    assert_eq!(
        aplicado, 1.0,
        "0,51 vira um ponto inteiro — o outro lado do mesmo problema"
    );
}

/// **A mesma medição, sobre população ENVELHECIDA.** Roda com:
///
///   cargo test --manifest-path src-tauri/Cargo.toml anulacao_em_carreira -- --nocapture
///
/// Por que este teste existe, e o erro que ele conserta. A medição irmã acima amostra o grid
/// RECÉM-GERADO, e para a motivação isso é a população errada: `motivation_pace_delta` é zero
/// por construção em `MOTIVATION_REF = 70` (há teste afirmando isso), o déficit é
/// `(70−m)/70 × 2,5`, e `driver_generation` sorteia motivação em `roll_stat(50, 80)` — ou seja,
/// **a população nasce centrada exatamente no ponto onde o elo está desligado de propósito**.
///
/// O limiar de sobrevivência ao arredondamento é `(70−m)/70 × 2,5 ≥ 0,5`, isto é `m ≤ 56`. A
/// geração põe só a faixa [50, 56] ali — ~10% da população, que é precisamente a anulação de
/// ~92% medida no outro teste. O número estava certo e a leitura errada: ele mede o instante
/// zero de uma carreira, não uma carreira.
///
/// Aqui a motivação é movida pela função REAL de evolução
/// (`evolution::motivation::adjust_end_of_season_motivation`), com desfechos de temporada
/// espalhados como numa categoria de verdade — campeão, top 3, meio de pelotão, rebaixado, quem
/// perdeu a vaga. Mesmo padrão de usar o gerador real em vez de inventar distribuição.
#[test]
fn anulacao_em_carreira_viva_mostra_elo_condicional_e_nao_morto() {
    use crate::evolution::growth::SeasonStats;
    use crate::evolution::motivation::{adjust_end_of_season_motivation, MotivationContext};
    use rand::{rngs::StdRng, SeedableRng};
    use std::collections::HashSet;

    let mut rng = StdRng::seed_from_u64(20260731);
    let mut nomes = HashSet::new();
    let mut pilotos = Driver::generate_for_category("gt3", 4, "medio", 24, &mut nomes, &mut rng);
    let total = pilotos.len() as i32;

    // Cinco temporadas. A posição de campeonato é PERSISTENTE — ordenada pelo skill, com um
    // tranco pequeno por ano —, e é isso que importa: numa carreira de verdade quem está no
    // fundo tende a ficar no fundo, e a desmotivação se ACUMULA. Rodar a posição entre todos
    // (a minha primeira tentativa) deixava cada piloto ser campeão uma vez e empurrava a
    // população inteira para cima, o que é justamente o vício de amostragem que este teste
    // existe para não repetir.
    let mut ordem: Vec<usize> = (0..pilotos.len()).collect();
    ordem.sort_by(|a, b| {
        pilotos[*b]
            .atributos
            .skill
            .partial_cmp(&pilotos[*a].atributos.skill)
            .unwrap()
    });
    let mut posto = vec![0i32; pilotos.len()];
    for (rank, idx) in ordem.iter().enumerate() {
        posto[*idx] = rank as i32 + 1;
    }

    for temporada in 0..5i32 {
        for (i, d) in pilotos.iter_mut().enumerate() {
            let tranco = (i as i32 + temporada * 3) % 5 - 2; // −2..+2
            let posicao = (posto[i] + tranco).clamp(1, total);
            let stats = SeasonStats {
                posicao_campeonato: posicao,
                total_pilotos: total,
                pontos: 0,
                vitorias: 0,
                podios: 0,
                corridas: 20,
                dnfs: if posicao > total - 6 { 3 } else { 1 },
            };
            let ctx = MotivationContext {
                was_champion: posicao == 1,
                was_promoted: posicao <= 2,
                was_relegated: posicao > total - 3,
                contract_renewed: posicao <= total / 2,
                lost_seat: posicao > total - 2,
                seasons_in_category: temporada + 1,
                outperformed_machinery: posicao <= 4,
            };
            adjust_end_of_season_motivation(d, &stats, &ctx, &mut rng);
        }
    }

    let mut motivacoes: Vec<f64> = pilotos.iter().map(|d| d.motivacao).collect();
    motivacoes.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!(
        "\n=== Motivação depois de 5 temporadas: min {:.0} | p25 {:.0} | mediana {:.0} | \
         p75 {:.0} | max {:.0} ===",
        motivacoes[0],
        motivacoes[motivacoes.len() / 4],
        motivacoes[motivacoes.len() / 2],
        motivacoes[motivacoes.len() * 3 / 4],
        motivacoes[motivacoes.len() - 1]
    );

    // A esteira sobre essa população, com todo o resto neutro para isolar a motivação.
    let grid: Vec<SimDriver> = pilotos
        .iter()
        .map(|d| {
            let team = placeholder_team_from_db(
                format!("T{}", d.id),
                "E".to_string(),
                "gt3".to_string(),
                "2026-01-01T00:00:00".to_string(),
            );
            SimDriver::from_driver_and_team(d, &team)
        })
        .collect();
    let ctxs: Vec<ContextoDoPiloto> = grid.iter().map(|_| ctx_pista_dominada()).collect();
    let estado = vec![0.0; grid.len()];
    let r = aplicar_esteira(&grid, &ctxs, 6, 5, 523, &estado, &Default::default());

    // Reparte por FAIXA, e as faixas saem da própria função — não são escolhidas a olho.
    // Déficit sobrevive ao arredondamento quando `(70−m)/70 × 2,5 ≥ 0,5`, isto é `m ≤ 56`.
    // Superávit sobrevive quando `(m−70)/30 × 0,8 ≥ 0,5`, isto é `m ≥ 88,75`.
    // Entre os dois há uma ZONA MUDA de largura ~33, centrada na referência 70, onde o elo é
    // silencioso de propósito. Tratar tudo acima de 56 como "neutro" foi o meu segundo erro de
    // amostragem neste mesmo teste: o superávit também tem limiar.
    const LIMIAR_DEFICIT: f64 = 56.0;
    const LIMIAR_SUPERAVIT: f64 = 88.75;

    let mut deficit = (0usize, 0usize); // (casos, anulados)
    let mut muda = (0usize, 0usize);
    let mut superavit = (0usize, 0usize);
    for (d, piloto) in r.deltas.iter().zip(&pilotos) {
        let anulado = d.aplicado_de(PassoDeArredondamento::Motivacao, Canal::Corrida) == 0.0;
        let alvo = if piloto.motivacao <= LIMIAR_DEFICIT {
            &mut deficit
        } else if piloto.motivacao >= LIMIAR_SUPERAVIT {
            &mut superavit
        } else {
            &mut muda
        };
        alvo.0 += 1;
        alvo.1 += usize::from(anulado);
    }

    let pct = |(casos, anulados): (usize, usize)| {
        if casos == 0 {
            f64::NAN
        } else {
            anulados as f64 / casos as f64 * 100.0
        }
    };
    println!(
        "déficit  (m ≤ {LIMIAR_DEFICIT:.0}): {:2} pilotos, ANULADO em {:3.0}%\n\
         zona muda ({LIMIAR_DEFICIT:.0} < m < {LIMIAR_SUPERAVIT}): {:2} pilotos, ANULADO em {:3.0}%\n\
         superávit (m ≥ {LIMIAR_SUPERAVIT}): {:2} pilotos, ANULADO em {:3.0}%",
        deficit.0,
        pct(deficit),
        muda.0,
        pct(muda),
        superavit.0,
        pct(superavit)
    );

    // O veredito: o elo é CONDICIONAL, não morto. Onde ele foi desenhado para agir, ele age; e
    // a zona muda é desenho, não defeito (`motivacao_referencia_sem_efeito` afirma isso).
    assert!(
        deficit.0 > 0,
        "a população envelhecida tinha que produzir desmotivados — sem eles o teste não mede nada"
    );
    assert_eq!(
        pct(deficit),
        0.0,
        "no desmotivado o elo tem que passar o arredondamento inteiro"
    );
    if muda.0 > 0 {
        assert_eq!(
            pct(muda),
            100.0,
            "na zona muda o elo é silencioso por construção"
        );
    }
    if superavit.0 > 0 {
        assert_eq!(
            pct(superavit),
            0.0,
            "no superávit alto o elo também sobrevive"
        );
    }
}

#[test]
fn elos_mapeiam_para_seis_passos_de_arredondamento() {
    use std::collections::HashSet;
    let passos: HashSet<PassoDeArredondamento> = [
        EloDaEsteira::ConhecimentoDePista,
        EloDaEsteira::AdaptacaoDeCategoria,
        EloDaEsteira::Lesao,
        EloDaEsteira::AfinidadeDePista,
        EloDaEsteira::FormaDoMomento,
        EloDaEsteira::AcertoDeFimDeSemana,
        EloDaEsteira::Motivacao,
        EloDaEsteira::Pressao,
    ]
    .iter()
    .map(|e| e.passo())
    .collect();
    assert_eq!(passos.len(), 6, "oito elos, seis arredondamentos");
    // E as três camadas de forma caem no MESMO passo — é o que preserva a resolução.
    assert_eq!(
        EloDaEsteira::AfinidadeDePista.passo(),
        EloDaEsteira::AcertoDeFimDeSemana.passo()
    );
    assert_eq!(
        EloDaEsteira::FormaDoMomento.passo(),
        PassoDeArredondamento::CamadasDeForma
    );
}
