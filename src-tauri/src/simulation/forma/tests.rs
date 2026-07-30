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
fn animo_desloca_a_media_da_forma() {
    // Piloto animado passa mais tempo em fase boa; desanimado, o contrário.
    let media = |mot: f64, conf: f64| {
        let mut f = 0.0;
        let mut soma = 0.0;
        for rodada in 0..400 {
            f = proxima_forma(f, semente_forma(9, rodada, "DRV-ANIMO"), mot, conf);
            soma += f;
        }
        soma / 400.0
    };
    let alto = media(95.0, 95.0);
    let baixo = media(10.0, 10.0);
    assert!(alto > baixo, "alto={alto} baixo={baixo}");
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
    let delta = a.classificacao - a.corrida;
    assert!(
        (delta - afinidade * (MULT_AFINIDADE_QUALI - 1.0)).abs() < 1e-9,
        "delta={delta} afinidade={afinidade}"
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
