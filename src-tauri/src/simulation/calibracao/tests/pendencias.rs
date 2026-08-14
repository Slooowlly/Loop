//! MEDIÇÃO das pendências B5 (semente da estratégia de parada) e B9 (grid × chegada).
//!
//! Régua, não conserto: nenhuma constante é tocada e nada de produção muda de comportamento.
//! O adaptador `atrito::posicoes_por_segmento` continua devolvendo `None` na árvore — a ligação
//! do dado acontece SÓ aqui, em [`posicoes_do_harness`], para que o guard
//! `adaptador_de_posicoes_por_segmento_ainda_esta_pendente` continue sendo o lembrete que ele é.
//!
//! ```text
//! cargo test --release --lib calibracao::tests::pendencias -- --ignored --nocapture
//! ```

use crate::simulation::calibracao::arena::{self, ConfigTemporada};
use crate::simulation::calibracao::campo::gerar_campo;
use crate::simulation::calibracao::{atrito, metricas, processo};
use crate::simulation::race::{planejar_paradas, RaceResult};

// ───────────────────────────────── B5 ─────────────────────────────────

/// Réplica de `simulation::race::estrategia::semente` (privada), para o contrafactual poder
/// acrescentar a identidade do evento sem tocar produção.
fn semente_replicada(partes: &[&str], numeros: &[i64]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325_u64;
    for p in partes {
        for b in p.bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01B3);
        }
    }
    for n in numeros {
        for b in n.to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01B3);
        }
    }
    h = (h ^ (h >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    h ^ (h >> 31)
}

fn uniforme(h: u64) -> f64 {
    (h >> 11) as f64 / (1u64 << 53) as f64
}

/// A chamada da equipe do CONTRAFACTUAL: mesma matemática de `planejar_paradas`, com
/// `temporada` e `rodada` acrescentados à semente. Nada mais muda.
fn planejar_com_identidade_do_evento(
    driver_id: &str,
    team_id: &str,
    track_id: u32,
    total_laps: i32,
    duracao_min: i32,
    qualidade: f64,
    temporada: i32,
    rodada: i32,
) -> (String, u32) {
    const MINUTOS_MINIMOS: i32 = 40;
    const JANELA: (f64, f64) = (0.35, 0.65);
    if duracao_min < MINUTOS_MINIMOS || total_laps < 8 {
        return ("sem-parada".to_string(), 0);
    }
    let h = semente_replicada(
        &[team_id, driver_id],
        &[
            track_id as i64,
            total_laps as i64,
            temporada as i64,
            rodada as i64,
        ],
    );
    let acerto = (qualidade.clamp(0.0, 100.0) / 100.0).clamp(0.0, 1.0);
    let desvio = (uniforme(h) * 2.0 - 1.0) * (1.0 - acerto);
    let (inicio, fim) = JANELA;
    let meio = (inicio + fim) / 2.0;
    let meia_largura = (fim - inicio) / 2.0;
    let fracao = (meio + desvio * meia_largura).clamp(inicio, fim);
    let volta = ((total_laps as f64 * fracao).round() as u32).max(2);
    let terco = (fracao - inicio) / (fim - inicio);
    let rotulo = if terco < 0.33 {
        "1-parada-cedo"
    } else if terco < 0.67 {
        "1-parada-ideal"
    } else {
        "1-parada-tarde"
    };
    (rotulo.to_string(), volta)
}

/// **B5** — a semente da chamada de parada não vê temporada nem rodada. Quanto isso repete.
#[test]
#[ignore = "medição B5; roda com --ignored --nocapture"]
fn b5_semente_da_estrategia_de_parada() {
    println!("\n===== B5 — SEMENTE DA ESTRATÉGIA DE PARADA =====");
    println!(
        "  hoje: `semente(&[team_id, driver_id], &[track_id, total_laps])`\n  \
         (simulation/race/estrategia.rs:177). Temporada e rodada não entram."
    );

    const TEMPORADAS: usize = 8;
    let config = ConfigTemporada::gt3(); // 45 min: acima do gate de 40, então há parada.
    let grid = gerar_campo(&config.perfil, config.pilotos, 4242);

    // (piloto, pista) → lista de (temporada, rodada, rótulo, volta) sob cada regime.
    let mut atual: std::collections::HashMap<(String, u32), Vec<(String, u32)>> =
        std::collections::HashMap::new();
    let mut contra: std::collections::HashMap<(String, u32), Vec<(String, u32)>> =
        std::collections::HashMap::new();
    let mut rotulos_atual: std::collections::HashMap<String, u32> =
        std::collections::HashMap::new();
    let mut rotulos_contra: std::collections::HashMap<String, u32> =
        std::collections::HashMap::new();
    let mut chamadas = 0usize;
    let mut mudou = 0usize;

    for t in 0..TEMPORADAS {
        let semente = arena::semente_da_temporada(4242, t);
        let eventos = arena::sortear_eventos(&config, semente);
        for (indice, evento) in eventos.iter().enumerate() {
            let rodada = indice + 1;
            let ctx = arena::contexto_do_evento(&config, rodada, evento);
            for d in &grid {
                let plano = planejar_paradas(
                    &d.id,
                    &d.team_id,
                    ctx.track_id,
                    ctx.total_laps,
                    ctx.race_duration_minutes,
                    d.qualidade_de_estrategia,
                );
                let volta_atual = plano.voltas_planejadas.first().copied().unwrap_or(0);
                let (rot_c, volta_c) = planejar_com_identidade_do_evento(
                    &d.id,
                    &d.team_id,
                    ctx.track_id,
                    ctx.total_laps,
                    ctx.race_duration_minutes,
                    d.qualidade_de_estrategia,
                    t as i32 + 1,
                    rodada as i32,
                );
                chamadas += 1;
                if volta_atual != volta_c {
                    mudou += 1;
                }
                *rotulos_atual
                    .entry(plano.estrategia_id.clone())
                    .or_insert(0) += 1;
                *rotulos_contra.entry(rot_c.clone()).or_insert(0) += 1;
                atual
                    .entry((d.id.clone(), ctx.track_id))
                    .or_default()
                    .push((plano.estrategia_id.clone(), volta_atual));
                contra
                    .entry((d.id.clone(), ctx.track_id))
                    .or_default()
                    .push((rot_c, volta_c));
            }
        }
    }

    // Repetição: entre as visitas de um mesmo (piloto, pista), quantos pares são idênticos.
    let taxa_de_repeticao =
        |mapa: &std::collections::HashMap<(String, u32), Vec<(String, u32)>>| {
            let (mut pares, mut iguais) = (0usize, 0usize);
            for visitas in mapa.values() {
                for i in 0..visitas.len() {
                    for j in (i + 1)..visitas.len() {
                        pares += 1;
                        if visitas[i] == visitas[j] {
                            iguais += 1;
                        }
                    }
                }
            }
            (pares, iguais)
        };
    let (pares_a, iguais_a) = taxa_de_repeticao(&atual);
    let (pares_c, iguais_c) = taxa_de_repeticao(&contra);

    println!(
        "\n  {TEMPORADAS} temporadas × {} etapas × {} pilotos = {chamadas} chamadas",
        config.etapas, config.pilotos
    );
    println!("\n{:>28} {:>12} {:>12}", "regime", "revisitas", "idênticas");
    println!(
        "{:>28} {pares_a:>12} {:>11.1}%",
        "atual (sem evento)",
        100.0 * iguais_a as f64 / pares_a.max(1) as f64
    );
    println!(
        "{:>28} {pares_c:>12} {:>11.1}%",
        "contrafactual (com evento)",
        100.0 * iguais_c as f64 / pares_c.max(1) as f64
    );
    println!(
        "\n  chamadas que MUDAM de volta ao incluir a identidade do evento: {mudou}/{chamadas} \
         ({:.1}%)",
        100.0 * mudou as f64 / chamadas.max(1) as f64
    );

    println!("\n-- distribuição dos rótulos de estratégia no grid --");
    println!("{:>18} {:>12} {:>16}", "rótulo", "atual", "contrafactual");
    let mut chaves: Vec<&String> = rotulos_atual.keys().collect();
    chaves.sort();
    let total = chamadas.max(1) as f64;
    for k in chaves {
        println!(
            "{k:>18} {:>11.1}% {:>15.1}%",
            100.0 * f64::from(*rotulos_atual.get(k).unwrap_or(&0)) / total,
            100.0 * f64::from(*rotulos_contra.get(k).unwrap_or(&0)) / total,
        );
    }
    println!(
        "\n  reprodutibilidade: as duas versões são deterministas — a chamada continua fora do\n  \
         `rng` da corrida, então a prova de equivalência do motor segue valendo. O que a versão\n  \
         contrafactual quebra é a SEQUÊNCIA ANTIGA: um save já em andamento passa a ver outra\n  \
         volta de parada nas etapas que ainda não correu."
    );
}

// ───────────────────────────────── B9 ─────────────────────────────────

/// O adaptador de produção, agora ligado. Era uma cópia local enquanto
/// `atrito::posicoes_por_segmento` devolvia `None` na árvore.
fn posicoes_do_harness(corrida: &RaceResult) -> Option<Vec<(String, Vec<i32>)>> {
    atrito::posicoes_por_segmento(corrida)
}

/// **B9** — grid × chegada com o dado de segmento ligado no harness.
#[test]
#[ignore = "medição B9; roda com --ignored --nocapture"]
fn b9_grid_contra_chegada() {
    println!("\n===== B9 — GRID × CHEGADA =====");
    println!(
        "  o campo `RaceDriverResult::posicoes_por_segmento` JÁ existe e vem preenchido de\n  \
         `simulation::race::motor::fechar_o_trecho`. Quem devolve `None` é só o adaptador\n  \
         `calibracao::atrito::posicoes_por_segmento` (atrito.rs:55), que continua intocado."
    );

    const TEMPORADAS: usize = 6;
    for (rotulo, config, semente) in [
        ("mazda_rookie", ConfigTemporada::rookie(), 2026u64),
        ("gt3", ConfigTemporada::gt3(), 2027),
    ] {
        let catalogo = arena::catalogo_para(&config);
        let mut temporadas = Vec::new();
        let mut processos = Vec::new();
        let mut rho_seg = [0.0f64; atrito::SEGMENTOS];
        let mut trocas_seg = [0.0f64; atrito::SEGMENTOS];
        let mut estabilizacao = 0.0;
        let mut travado = 0.0;
        let mut comboio = 0.0;
        let mut amostras_seg = 0.0;
        let (mut tentativas, mut concluidas, mut sofridas) = (0.0f64, 0.0f64, 0.0f64);
        let mut corridas_totais = 0.0;
        let mut ligou = 0usize;

        for t in 0..TEMPORADAS {
            let s = arena::semente_da_temporada(semente, t);
            let grid = gerar_campo(&config.perfil, config.pilotos, s);
            let corridas = arena::rodar_temporada(&config, &grid, &catalogo, s, t as i32 + 1);
            temporadas.push(metricas::medir_temporada(&grid, &corridas));
            for corrida in &corridas {
                corridas_totais += 1.0;
                processos.push(processo::medir_processo(corrida));
                for r in &corrida.race_results {
                    tentativas += f64::from(r.tentativas_ultrapassagem);
                    concluidas += f64::from(r.ultrapassagens_concluidas);
                    sofridas += f64::from(r.tentativas_sofridas);
                }
                let Some(posicoes) = posicoes_do_harness(corrida) else {
                    continue;
                };
                ligou += 1;
                let grid_inicial: Vec<i32> = posicoes
                    .iter()
                    .map(|(id, _)| {
                        corrida
                            .race_results
                            .iter()
                            .find(|r| &r.pilot_id == id)
                            .map(|r| r.grid_position)
                            .unwrap_or(0)
                    })
                    .collect();
                let m = atrito::medir_segmentos(&grid, &posicoes, &grid_inicial);
                for s in 0..atrito::SEGMENTOS {
                    rho_seg[s] += m.rho_por_segmento[s];
                    trocas_seg[s] += m.trocas_por_segmento[s];
                }
                estabilizacao += m.segmento_de_estabilizacao;
                travado += m.fracao_travado;
                comboio += m.maior_sequencia_travado;
                amostras_seg += 1.0;
            }
        }

        let agregado = metricas::agregar(rotulo, &temporadas);
        let proc = processo::agregar_processo(rotulo, &processos);
        println!("\n── {rotulo} ({TEMPORADAS} temporadas, {corridas_totais:.0} corridas) ──");
        println!(
            "  ρ(grid × chegada)          {:.3}",
            agregado.spearman_grid_chegada
        );
        println!(
            "  pole → vitória             {:.1}%",
            100.0 * agregado.pct_vitorias_do_pole
        );
        println!(
            "  ρ(etapas consecutivas)     {:.3}",
            agregado.spearman_etapas_consecutivas
        );
        println!(
            "  vencedores distintos       {:.2}",
            agregado.vencedores_distintos
        );
        println!(
            "  desvio de posição          {:.2}",
            agregado.desvio_posicao
        );
        println!(
            "  ganho/perda médio |Δpos|   {:.2}   (p90 {:.2}, maior {:.2})",
            proc.ganho_medio_abs, proc.ganho_p90, proc.maior_ganho
        );
        println!(
            "  trocas normalizadas        {:.3}   (0 = chegou na ordem que largou)",
            proc.trocas_normalizadas
        );
        println!(
            "  ultrapassagens             {:.1} tentativas/corrida, {:.1} concluídas ({:.0}% de \
             sucesso), {:.1} sofridas",
            tentativas / corridas_totais,
            concluidas / corridas_totais,
            100.0 * concluidas / tentativas.max(1.0),
            sofridas / corridas_totais,
        );
        println!("  adaptador ligou em          {ligou}/{corridas_totais:.0} corridas");
        if amostras_seg > 0.0 {
            println!("\n  curva ρ(segmento × chegada) — o dado que faltava:");
            println!(
                "    {:>10} {:>10} {:>10} {:>10} {:>10}",
                "Start", "Early", "Mid", "Late", "Finish"
            );
            print!("    ");
            for s in 0..atrito::SEGMENTOS {
                print!("{:>10.3} ", rho_seg[s] / amostras_seg);
            }
            println!();
            println!("  trocas (Kendall) DENTRO de cada segmento:");
            print!("    ");
            for s in 0..atrito::SEGMENTOS {
                print!("{:>10.1} ", trocas_seg[s] / amostras_seg);
            }
            println!();
            println!(
                "  segmento de estabilização  {:.2}   (limiar ρ ≥ {:.2})",
                estabilizacao / amostras_seg,
                atrito::LIMIAR_DE_ESTABILIZACAO
            );
            println!(
                "  fração travado             {:.3}   maior comboio {:.2} segmentos",
                travado / amostras_seg,
                comboio / amostras_seg
            );
        }
    }

    // A sonda que separa "o grid manda" de "o ritmo manda".
    println!("\n-- sonda de grade sorteada (quem realmente causa a previsibilidade) --");
    for (rotulo, config, semente) in [
        ("mazda_rookie", ConfigTemporada::rookie(), 2026u64),
        ("gt3", ConfigTemporada::gt3(), 2027),
    ] {
        let p = processo::medir_poder_da_largada(rotulo, &config, 30, semente);
        println!(
            "  {rotulo:<14} ρ(grid normal) {:.3} | ρ(grid sorteado) {:.3} | ρ(skill|sorteado) \
             {:.3} | trocas {:.3}",
            p.rho_grid_normal,
            p.rho_grid_sorteado,
            p.rho_skill_com_grid_sorteado,
            p.trocas_normalizadas_sorteado
        );
    }
    println!(
        "  leitura: ρ(grid sorteado) alto = a posição de largada em si persiste (atrito de\n  \
         pista). ρ(skill|sorteado) alto com ρ(grid sorteado) baixo = quem manda é o ritmo, e a\n  \
         previsibilidade vem da CLASSIFICAÇÃO, não da corrida."
    );
}

/// **B9 — a varredura que escolhe o valor.** Varre os dois knobs com gradiente conhecido
/// (`prob_base_ultrapassagem` e `delta_de_ritmo_que_satura`) contra os critérios de saída de
/// B9, nas DUAS categorias, e imprime tudo o que é preciso para decidir sem abrir outra
/// medição: o grid contra a chegada, a vitória do pole, a taxa de sucesso das ultrapassagens,
/// e os dois contrapesos (poder explicativo do skill e taxa de abandono).
///
/// ```text
/// cargo test --release --lib pendencias::b9_varredura -- --ignored --nocapture
/// ```
#[test]
#[ignore = "varredura B9; roda com --ignored --nocapture"]
fn b9_varredura_de_ultrapassagem() {
    use crate::simulation::calibracao::arena::{AjustesCtx, AjustesTrafego};
    use crate::simulation::race::trafego::ParametrosDeTrafego;

    const TEMPORADAS: usize = 24;
    let padrao = ParametrosDeTrafego::PADRAO;

    println!("\n===== B9 — VARREDURA DE ULTRAPASSAGEM =====");
    println!(
        "  hoje: prob_base = {:.2}, delta_que_satura = {:.1}\n  \
         alvo rookie: rho <= 0,88 e pole <= 70% | alvo gt3: rho <= 0,93 e pole <= 78%\n  \
         'rho skill' e o poder explicativo do ritmo: nao pode cair.",
        padrao.prob_base_ultrapassagem, padrao.delta_de_ritmo_que_satura
    );

    let pontos: &[(f64, f64)] = &[
        (
            padrao.prob_base_ultrapassagem,
            padrao.delta_de_ritmo_que_satura,
        ),
        (0.50, 6.0),
        (0.65, 6.0),
        (0.80, 6.0),
        (0.35, 3.0),
        (0.50, 3.0),
        (0.65, 3.0),
        (0.80, 3.0),
        (0.65, 2.0),
        (0.80, 2.0),
        (0.90, 2.0),
        (0.90, 1.5),
        (0.95, 1.5),
        (0.95, 1.0),
    ];

    for (rotulo, base_config, semente) in [
        ("mazda_rookie", ConfigTemporada::rookie(), 2026u64),
        ("gt3", ConfigTemporada::gt3(), 2027),
    ] {
        println!("\n── {rotulo} ──");
        println!(
            "{:>10} {:>10} | {:>9} {:>9} {:>9} | {:>9} {:>9} {:>8} {:>8} {:>8}",
            "prob",
            "satura",
            "rho grid",
            "pole%",
            "sucesso%",
            "rho skill",
            "rho N,N+1",
            "|dpos|",
            "recup",
            "dnf/et"
        );
        println!("{}", "-".repeat(112));
        for (prob, satura) in pontos {
            let config = ConfigTemporada {
                ajustes: AjustesCtx {
                    trafego: AjustesTrafego {
                        prob_base_ultrapassagem: Some(*prob),
                        delta_de_ritmo_que_satura: Some(*satura),
                        ..AjustesTrafego::default()
                    },
                    ..base_config.ajustes
                },
                ..base_config.clone()
            };
            let catalogo = arena::catalogo_para(&config);
            let mut temporadas = Vec::new();
            let mut processos = Vec::new();
            let (mut tentativas, mut concluidas) = (0.0f64, 0.0f64);
            let (mut largadas, mut abandonos) = (0.0f64, 0.0f64);
            let mut recuperacao = 0.0f64;
            let mut corridas_totais = 0.0f64;
            // ρ(skill × chegada) por corrida: o poder explicativo do ritmo, que é o contrapeso
            // de B9. `MetricasAgregadas` não tem esta coluna, então ela sai daqui.
            let (mut rho_skill, mut rho_skill_n) = (0.0f64, 0.0f64);

            for t in 0..TEMPORADAS {
                let s = arena::semente_da_temporada(semente, t);
                let grid = gerar_campo(&config.perfil, config.pilotos, s);
                let corridas = arena::rodar_temporada(&config, &grid, &catalogo, s, t as i32 + 1);
                temporadas.push(metricas::medir_temporada(&grid, &corridas));
                for corrida in &corridas {
                    corridas_totais += 1.0;
                    processos.push(processo::medir_processo(corrida));
                    recuperacao += f64::from(
                        corrida
                            .race_results
                            .iter()
                            .filter(|r| !r.is_dnf)
                            .map(|r| r.positions_gained)
                            .max()
                            .unwrap_or(0),
                    );
                    for r in &corrida.race_results {
                        tentativas += f64::from(r.tentativas_ultrapassagem);
                        concluidas += f64::from(r.ultrapassagens_concluidas);
                        largadas += 1.0;
                        if r.is_dnf {
                            abandonos += 1.0;
                        }
                    }
                    let (skills, chegadas): (Vec<f64>, Vec<f64>) = corrida
                        .race_results
                        .iter()
                        .filter(|r| !r.is_dnf)
                        .filter_map(|r| {
                            grid.iter()
                                .find(|d| d.id == r.pilot_id)
                                // Sinal invertido: skill alto tem que casar com posição BAIXA,
                                // então a correlação sai positiva quando o ritmo manda.
                                .map(|d| (-(d.skill as f64), r.finish_position as f64))
                        })
                        .unzip();
                    if let Some(rho) = metricas::spearman(&skills, &chegadas) {
                        rho_skill += rho;
                        rho_skill_n += 1.0;
                    }
                }
            }

            let agregado = metricas::agregar(rotulo, &temporadas);
            let proc = processo::agregar_processo(rotulo, &processos);
            let marca = |v: f64, alvo: f64| if v <= alvo { "*" } else { " " };
            let (alvo_rho, alvo_pole) = if rotulo == "gt3" {
                (0.93, 0.78)
            } else {
                (0.88, 0.70)
            };
            println!(
                "{prob:>10.2} {satura:>10.1} | {:>8.3}{} {:>8.1}{} {:>9.1} | {:>9.3} {:>9.3} \
                 {:>8.2} {:>8.2} {:>8.3}",
                agregado.spearman_grid_chegada,
                marca(agregado.spearman_grid_chegada, alvo_rho),
                100.0 * agregado.pct_vitorias_do_pole,
                marca(agregado.pct_vitorias_do_pole, alvo_pole),
                100.0 * concluidas / tentativas.max(1.0),
                rho_skill / rho_skill_n.max(1.0),
                agregado.spearman_etapas_consecutivas,
                proc.ganho_medio_abs,
                recuperacao / corridas_totais.max(1.0),
                agregado.dnfs_por_etapa,
            );
            let _ = (abandonos, largadas);
        }
    }
    println!(
        "\n  '*' marca o ponto que cumpre o alvo daquela coluna. A escolha tem que cumprir os\n  \
         DOIS alvos nas DUAS categorias, sem derrubar 'rho skill' nem subir 'dnf%'."
    );
}

/// **B9 — segunda varredura: por que a gt3 não desce.** A primeira varredura mostrou que a
/// ultrapassagem sozinha leva a rookie ao alvo e deixa a gt3 parada em ~0,94. A hipótese é que
/// na gt3 o pelotão anda ESPALHADO, então a tentativa nem chega a acontecer: quem está fora da
/// janela de ataque não ataca, por mais provável que a manobra fosse. Esta varredura mexe nos
/// dois mecanismos que decidem se a tentativa EXISTE — a janela de ataque e a perda por ar
/// sujo —, sobre o melhor ponto de ultrapassagem da primeira.
///
/// ```text
/// cargo test --release --lib pendencias::b9_varredura_de_aproximacao -- --ignored --nocapture
/// ```
#[test]
#[ignore = "varredura B9; roda com --ignored --nocapture"]
fn b9_varredura_de_aproximacao() {
    use crate::simulation::calibracao::arena::{AjustesCtx, AjustesTrafego};
    use crate::simulation::race::trafego::ParametrosDeTrafego;

    const TEMPORADAS: usize = 6;
    let padrao = ParametrosDeTrafego::PADRAO;
    // O melhor ponto da primeira varredura, fixo aqui.
    const PROB: f64 = 0.90;
    const SATURA: f64 = 2.0;

    println!("\n===== B9 — VARREDURA DE APROXIMAÇÃO (janela de ataque × ar sujo) =====");
    println!(
        "  fixo: prob_base = {PROB:.2}, delta_que_satura = {SATURA:.1}\n  \
         hoje: janela_ataque = {:.0} ms, perda_ar_sujo = {:.1} pontos",
        padrao.janela_de_ataque_ms, padrao.perda_maxima_ar_sujo_pontos
    );

    let pontos: &[(f64, f64)] = &[
        (
            padrao.janela_de_ataque_ms,
            padrao.perda_maxima_ar_sujo_pontos,
        ),
        (1_400.0, 3.0),
        (2_000.0, 3.0),
        (2_000.0, 1.5),
        (2_800.0, 1.5),
        (2_800.0, 0.5),
        (4_000.0, 1.5),
        (4_000.0, 0.5),
    ];

    for (rotulo, base_config, semente) in [
        ("mazda_rookie", ConfigTemporada::rookie(), 2026u64),
        ("gt3", ConfigTemporada::gt3(), 2027),
    ] {
        println!("\n── {rotulo} ──");
        println!(
            "{:>10} {:>10} | {:>9} {:>9} {:>9} {:>9} | {:>9} {:>9} {:>8}",
            "janela",
            "ar sujo",
            "rho grid",
            "pole%",
            "tent/cor",
            "sucesso%",
            "rho skill",
            "rho N,N+1",
            "recup"
        );
        println!("{}", "-".repeat(106));
        for (janela, ar_sujo) in pontos {
            let config = ConfigTemporada {
                ajustes: AjustesCtx {
                    trafego: AjustesTrafego {
                        prob_base_ultrapassagem: Some(PROB),
                        delta_de_ritmo_que_satura: Some(SATURA),
                        janela_de_ataque_ms: Some(*janela),
                        perda_maxima_ar_sujo_pontos: Some(*ar_sujo),
                        ..AjustesTrafego::default()
                    },
                    ..base_config.ajustes
                },
                ..base_config.clone()
            };
            let catalogo = arena::catalogo_para(&config);
            let mut temporadas = Vec::new();
            let (mut tentativas, mut concluidas) = (0.0f64, 0.0f64);
            let (mut rho_skill, mut rho_skill_n) = (0.0f64, 0.0f64);
            let (mut recuperacao, mut corridas_totais) = (0.0f64, 0.0f64);

            for t in 0..TEMPORADAS {
                let s = arena::semente_da_temporada(semente, t);
                let grid = gerar_campo(&config.perfil, config.pilotos, s);
                let corridas = arena::rodar_temporada(&config, &grid, &catalogo, s, t as i32 + 1);
                temporadas.push(metricas::medir_temporada(&grid, &corridas));
                for corrida in &corridas {
                    corridas_totais += 1.0;
                    recuperacao += f64::from(
                        corrida
                            .race_results
                            .iter()
                            .filter(|r| !r.is_dnf)
                            .map(|r| r.positions_gained)
                            .max()
                            .unwrap_or(0),
                    );
                    for r in &corrida.race_results {
                        tentativas += f64::from(r.tentativas_ultrapassagem);
                        concluidas += f64::from(r.ultrapassagens_concluidas);
                    }
                    let (skills, chegadas): (Vec<f64>, Vec<f64>) = corrida
                        .race_results
                        .iter()
                        .filter(|r| !r.is_dnf)
                        .filter_map(|r| {
                            grid.iter()
                                .find(|d| d.id == r.pilot_id)
                                .map(|d| (-(d.skill as f64), r.finish_position as f64))
                        })
                        .unzip();
                    if let Some(rho) = metricas::spearman(&skills, &chegadas) {
                        rho_skill += rho;
                        rho_skill_n += 1.0;
                    }
                }
            }

            let agregado = metricas::agregar(rotulo, &temporadas);
            println!(
                "{janela:>10.0} {ar_sujo:>10.1} | {:>9.3} {:>9.1} {:>9.1} {:>9.1} | {:>9.3} \
                 {:>9.3} {:>8.2}",
                agregado.spearman_grid_chegada,
                100.0 * agregado.pct_vitorias_do_pole,
                tentativas / corridas_totais.max(1.0),
                100.0 * concluidas / tentativas.max(1.0),
                rho_skill / rho_skill_n.max(1.0),
                agregado.spearman_etapas_consecutivas,
                recuperacao / corridas_totais.max(1.0),
            );
        }
    }
}

// ───────────────────────── B19: o lado da simulação ─────────────────────────

/// **B19 (parte da simulação)** — o teto de voltas muda o resultado da corrida?
#[test]
#[ignore = "medição B19 no motor; roda com --ignored --nocapture"]
fn b19_teto_de_voltas_no_resultado() {
    println!("\n===== B19 — TETO DE VOLTAS: EFEITO NO RESULTADO =====");
    println!(
        "  `calendar::montagem::estimate_laps` fechava em `clamp(5, 50)` para toda duração;\n  \
         desde 12/08/2026 o teto é 50 até 60 min e 150 acima disso (B19).\n  \
         O motor de corrida roda 5 SEGMENTOS, não voltas: `total_laps` entra em\n  \
         `voltas_no_segmento`, `tempo_do_segmento_ms`, `atraso_de_largada_ms`, na volta da\n  \
         parada e em `estimate_laps_at_dnf`."
    );

    use crate::simulation::qualifying::simulate_qualifying;
    use crate::simulation::race::simulate_race_with_breakdowns;
    use crate::simulation::scoring::{assign_points, determine_fastest_lap};
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    const REPETICOES: usize = 40;
    let base = ConfigTemporada::gt3();
    let catalogo = arena::catalogo_para(&base);

    println!(
        "\n{:>10} {:>10} {:>14} {:>16} {:>16} {:>16}",
        "total_laps", "ρ grid", "|Δpos| médio", "trocas norm.", "voltas na janela", "parada média"
    );
    for &laps in &[25i32, 50, 75, 100, 150] {
        let (mut rho_soma, mut ganho, mut trocas, mut volta_soma, mut n) =
            (0.0, 0.0, 0.0, 0.0, 0.0);
        let mut paradas_contadas = 0.0f64;
        let mut voltas_distintas = std::collections::HashSet::new();
        for i in 0..REPETICOES {
            // Contrafactual PAREADO: mesma semente, mesmo grid, mesmo evento; muda só o
            // `total_laps` do contexto.
            let semente = arena::semente_da_temporada(9090, i);
            let grid = gerar_campo(&base.perfil, base.pilotos, semente);
            let eventos = arena::sortear_eventos(&base, semente);
            let Some(evento) = eventos.first() else {
                continue;
            };
            let mut ctx = arena::contexto_do_evento(&base, 1, evento);
            ctx.total_laps = laps;

            let mut rng = StdRng::seed_from_u64(semente ^ 0xB19);
            let qualifying = simulate_qualifying(&grid, &ctx, &mut rng);
            let mut corrida = simulate_race_with_breakdowns(
                &grid,
                &qualifying,
                &ctx,
                &catalogo,
                false,
                None,
                &mut rng,
            );
            let fastest = determine_fastest_lap(&mut corrida.race_results).unwrap_or_default();
            assign_points(&mut corrida.race_results, false);
            corrida.fastest_lap_id = fastest;

            let (g, f): (Vec<f64>, Vec<f64>) = corrida
                .race_results
                .iter()
                .filter(|r| !r.is_dnf)
                .map(|r| (r.grid_position as f64, r.finish_position as f64))
                .unzip();
            if let Some(rho) = metricas::spearman(&g, &f) {
                rho_soma += rho;
            }
            let p = processo::medir_processo(&corrida);
            ganho += p.ganho_medio_abs;
            trocas += p.trocas_normalizadas;
            for r in &corrida.race_results {
                for v in &r.volta_da_parada {
                    voltas_distintas.insert(*v);
                    volta_soma += f64::from(*v);
                    paradas_contadas += 1.0;
                }
            }
            n += 1.0;
        }
        println!(
            "{laps:>10} {:>10.3} {:>14.2} {:>16.3} {:>16} {:>16.1}",
            rho_soma / n,
            ganho / n,
            trocas / n,
            voltas_distintas.len(),
            volta_soma / paradas_contadas.max(1.0),
        );
    }
    println!(
        "  a coluna 'voltas na janela' conta quantas voltas DISTINTAS de parada o grid inteiro\n  \
         conseguiu escolher em {REPETICOES} corridas: é a granularidade real da estratégia."
    );
}
