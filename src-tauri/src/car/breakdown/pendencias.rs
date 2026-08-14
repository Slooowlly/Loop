//! MEDIÇÃO das pendências B11, B13, B19 e B20 — nenhuma asserção de comportamento.
//!
//! Este arquivo é régua, igual ao irmão `medicao.rs`: ele não muda nada do jogo, só imprime o
//! que os mecanismos fazem hoje nas durações reais do Endurance (120, 180, 240 e 360 min) e o
//! que fariam sob curvas contrafactuais. Nenhuma curva é aplicada.
//!
//! ```text
//! cargo test --release --lib pendencias -- --ignored --nocapture
//! ```

use super::*;
use crate::car::wear::wear_per_race;
use crate::car::{Car, PartType};

/// As durações reais que o calendário sorteia para o Endurance (`calendar::entry`).
const DURACOES_ENDURANCE: [u16; 4] = [120, 180, 240, 360];
/// Pista neutra e clima neutro: o eixo em medição é a DURAÇÃO, não a pista.
const TRACK: (f64, f64, f64) = (1.0, 1.0, 1.0);
const PIT_CREW: f64 = 50.0;
/// Grid do Endurance (`constants::categories`: 18 equipes × 2 pilotos).
const CARROS: u32 = 36;
/// Amostras por célula. 200 estabiliza a segunda casa das taxas medidas.
const AMOSTRAS: u32 = 200;

/// Desgaste de entrada de uma frota em regime: cada peça num ponto qualquer da própria vida.
/// Mesma suposição do `medicao.rs`, e a mesma ressalva: é suposta, não lida de um save.
fn carro_em_regime(indice: u32, rodada: u32) -> Car {
    let mut car = Car::uniform(5);
    for (i, peca) in car.parts.iter_mut().enumerate() {
        peca.wear = f64::from((indice * 37 + i as u32 * 61 + rodada * 7) % 100) / 100.0;
    }
    car
}

/// Voltas de uma etapa pela mesma conta de `calendar::montagem::estimate_laps` (linha 175).
/// Replicada porque o módulo `calendar::montagem` é privado; o `cap` é parâmetro para o B19
/// poder medir 50, 75, 100, 150 e sem teto.
fn voltas_estimadas(comprimento_km: f64, duracao_min: i32, cap: i32) -> i32 {
    let tempo_volta_estimado_min = comprimento_km / 2.0;
    ((duracao_min as f64 / tempo_volta_estimado_min).ceil() as i32).clamp(5, cap)
}

/// As voltas que o CALENDÁRIO montaria para esta etapa, com o teto de produção (B19).
/// É o que o B20 tem que usar: medir o card contra uma contagem de voltas que o jogo não
/// produz mede outra coisa.
fn voltas_da_producao(comprimento_km: f64, duracao_min: i32) -> i32 {
    voltas_estimadas(
        comprimento_km,
        duracao_min,
        crate::calendar::teto_de_voltas(duracao_min),
    )
}

/// Uma corrida do modelo de quebra: devolve (leves, graves, dnf, desgaste ACUMULADO por peça).
///
/// O acumulado soma os incrementos POSITIVOS volta a volta, e não o saldo `final − entrada`.
/// O saldo mente: peça que quebra é consertada e volta a desgaste baixo, então uma prova mais
/// longa termina com saldo MENOR justamente por ter gasto mais. A medição também sai de dentro
/// do `LiveBreakdown`, porque o desgaste que ele guarda já passou pela vida individual da peça
/// e pela proteção do time — comparar com `car.parts[].wear` mediria duas escalas diferentes.
fn rodar_corrida(
    car: &Car,
    voltas: u32,
    semente: u64,
    is_enduro: bool,
    service_laps: &[u32],
) -> (u32, u32, u32, [f64; 11]) {
    let mut state = LiveBreakdown::new(car, semente, PIT_CREW, TRACK)
        .with_enduro(is_enduro)
        .with_tent(true);
    let (mut leve, mut grave, mut dnf) = (0u32, 0u32, 0u32);
    let mut acumulado = [0.0f64; 11];
    for volta in 1..=voltas {
        if state.is_out() {
            break;
        }
        if service_laps.contains(&volta) {
            state.service_pit();
        }
        let antes = state.wear;
        let progresso = f64::from(volta) / f64::from(voltas.max(1));
        for ev in state.advance_lap_at(volta, Weather::NEUTRAL, progresso) {
            match ev.severity {
                Severity::Light => leve += 1,
                Severity::Heavy => grave += 1,
                Severity::Dnf => dnf += 1,
            }
        }
        for i in 0..11 {
            acumulado[i] += (state.wear[i] - antes[i]).max(0.0);
        }
    }
    (leve, grave, dnf, acumulado)
}

// ─────────────────────────────── B11 ───────────────────────────────

/// **B11** — o sobrecusto de peça do enduro nas quatro durações reais, o desgaste ao vivo, a
/// quebra, o custo em dinheiro e o efeito das paradas. Depois, as curvas contrafactuais.
#[test]
#[ignore = "medição B11; roda com --ignored --nocapture"]
fn b11_desgaste_enduro_nas_duracoes_reais() {
    println!("\n===== B11 — DESGASTE DE ENDURO POR DURAÇÃO =====");

    // ── 1. O multiplicador de economia, com e sem parada ──
    println!("\n-- multiplicador de desgaste na economia (1 + sobrecusto) --");
    println!(
        "{:>6} {:>10} {:>10} {:>10} {:>12}",
        "min", "sem parada", "paradas IA", "com alívio", "vida de peça"
    );
    for &min in &DURACOES_ENDURANCE {
        let d = DuracaoDeProva::constante(min);
        let pits = d.paradas_modeladas_da_ia();
        let cru = d.mult_de_desgaste_na_economia(0);
        let aliviado = d.mult_de_desgaste_na_economia(pits);
        // Vida consumida da peça mais frágil (durabilidade 3 → 1/3 por etapa).
        let vidas = aliviado * wear_per_race(PartType::Engine);
        println!("{min:>6} {cru:>10.3} {pits:>10} {aliviado:>10.3} {vidas:>12.3}");
    }
    println!(
        "  o alívio de parada é o ÚNICO eixo vivo acima de 80 min: o sobrecusto cru já está no\n  \
         teto ({:.1}) desde os 80 min, então 120, 180, 240 e 360 compartilham o mesmo 3,0×.",
        super::enduro::ENDURO_SURCHARGE_CAP
    );

    // ── 2. Desgaste ao vivo por peça, quebra e custo ──
    println!("\n-- corrida ao vivo: desgaste médio por peça, quebras e custo --");
    println!(
        "{:>6} {:>7} {:>9} {:>9} {:>10} {:>9} {:>13}",
        "min", "voltas", "Δwear", "leves/car", "graves/car", "dnf %", "custo etapa"
    );
    // Pista de 5,0 km (mediana do catálogo) para converter minutos em voltas.
    let mut delta_por_duracao: Vec<(u16, [f64; 11])> = Vec::new();
    for &min in &DURACOES_ENDURANCE {
        let voltas = voltas_da_producao(5.0, i32::from(min)) as u32;
        let d = DuracaoDeProva::constante(min);
        let (mut l, mut g, mut dn) = (0u32, 0u32, 0u32);
        let mut soma = [0.0f64; 11];
        for i in 0..AMOSTRAS {
            let car = carro_em_regime(i % CARROS + 1, i / CARROS);
            let (a, b, c, delta) = rodar_corrida(&car, voltas, u64::from(i) * 977 + 13, true, &[]);
            l += a;
            g += b;
            dn += c;
            for k in 0..11 {
                soma[k] += delta[k];
            }
        }
        let n = f64::from(AMOSTRAS);
        let mut medio = [0.0f64; 11];
        for k in 0..11 {
            medio[k] = soma[k] / n;
        }
        delta_por_duracao.push((min, medio));
        // Custo de peça da etapa: preço × fração de vida consumida × multiplicador de enduro.
        let mult = d.mult_de_desgaste_na_economia(d.paradas_modeladas_da_ia());
        let custo: f64 = PartType::ALL
            .iter()
            .map(|&p| crate::car::cost::part_cost("endurance", p, 5) * wear_per_race(p) * mult)
            .sum();
        println!(
            "{min:>6} {voltas:>7} {:>9.3} {:>9.2} {:>10.2} {:>8.1}% {custo:>13.0}",
            medio.iter().sum::<f64>() / 11.0,
            f64::from(l) / n,
            f64::from(g) / n,
            100.0 * f64::from(dn) / n,
        );
    }
    println!(
        "  o desgaste AO VIVO e a quebra separam as durações só até o teto de voltas morder; o\n  \
         custo NÃO separa em nenhum ponto, porque vem do multiplicador saturado."
    );

    println!("\n-- desgaste acumulado POR PEÇA (fração de vida), por duração --");
    print!("{:>14}", "peça");
    for (min, _) in &delta_por_duracao {
        print!(" {:>8}", format!("{min} min"));
    }
    println!(" {:>8}", "durab.");
    for (k, &p) in PartType::ALL.iter().enumerate() {
        print!("{:>14}", format!("{p:?}"));
        for (_, medio) in &delta_por_duracao {
            print!(" {:>8.3}", medio[k]);
        }
        println!(" {:>8}", p.durability());
    }

    // ── 3. Curvas contrafactuais ──
    println!("\n-- curvas contrafactuais (nenhuma aplicada) --");
    println!(
        "  invariante: mult = 1 + sobrecusto ≤ 3,0, porque a peça mais frágil tem durabilidade 3\n  \
         (1/3 de vida por etapa) e uma prova única não pode consumir mais que uma vida inteira."
    );
    let k = super::enduro::ENDURO_COST_K;
    let gate = super::enduro::ENDURO_DURATION_GATE_MIN;
    let teto = super::enduro::ENDURO_SURCHARGE_CAP;
    println!(
        "\n{:>6} {:>9} {:>9} {:>9} {:>9}",
        "min", "atual", "A: raiz", "B: log", "C: satur."
    );
    for &min in &[60u16, 80, 120, 180, 240, 360] {
        let over = (f64::from(min) - gate).max(0.0) / gate;
        let atual = (k * over).min(teto);
        // A — raiz: preserva 60 min → 1,0 e cresce devagar; chega a 2,0 só em 360.
        // sobrecusto = teto * sqrt(over / over_360), over_360 = 8.0
        let a = teto * (over / 8.0).sqrt();
        // B — log: sobrecusto = teto * ln(1+over)/ln(1+8), mesma âncora em 360.
        let b = teto * (1.0 + over).ln() / 9.0f64.ln();
        // C — saturação exponencial: sobrecusto = teto * (1 - e^(-over/2)), assintótica ao teto.
        let c = teto * (1.0 - (-over / 2.0).exp());
        println!("{min:>6} {atual:>9.3} {a:>9.3} {b:>9.3} {c:>9.3}");
    }
    println!(
        "\n{:>6} {:>9} {:>9} {:>9} {:>9}   (multiplicador = 1 + sobrecusto)",
        "min", "atual", "A: raiz", "B: log", "C: satur."
    );
    for &min in &[60u16, 80, 120, 180, 240, 360] {
        let over = (f64::from(min) - gate).max(0.0) / gate;
        let atual = 1.0 + (k * over).min(teto);
        let a = 1.0 + teto * (over / 8.0).sqrt();
        let b = 1.0 + teto * (1.0 + over).ln() / 9.0f64.ln();
        let c = 1.0 + teto * (1.0 - (-over / 2.0).exp());
        println!("{min:>6} {atual:>9.3} {a:>9.3} {b:>9.3} {c:>9.3}");
        assert!(
            a <= 3.0 + 1e-9 && b <= 3.0 + 1e-9 && c <= 3.0 + 1e-9,
            "curva candidata violou o invariante da peça mais frágil"
        );
    }
    println!(
        "  as três respeitam o invariante e diferenciam 120 de 360. O que elas PERDEM é a\n  \
         calibração original: hoje 60 min → 2,0× e 80 min → 3,0× são pontos fixos, e só a\n  \
         curva atual passa por eles."
    );
}

// ─────────────────────────────── B13 ───────────────────────────────

/// **B13** — o `service_pit` ligado só aqui: risco de quebra com e sem serviço, nas durações
/// reais. Roda depois do B19 porque as voltas de cada cenário vêm de lá.
#[test]
#[ignore = "medição B13; roda com --ignored --nocapture"]
fn b13_efeito_do_service_pit() {
    println!("\n===== B13 — SERVICE_PIT (só no harness) =====");
    println!(
        "  dado real disponível hoje: `LiveBreakdown::service_pit` zera o desgaste de toda peça\n  \
         não quebrada acima de {SERVICE_WEAR_FLOOR:.2}; `BreakdownDirector::add_car` aceita as\n  \
         voltas de serviço; `forecast_breakdown_risk` aceita `service_laps`. Os DOIS call sites\n  \
         de produção (card do jogador e tabela do campeonato) passam `&[]`, e o disparo ao vivo\n  \
         recebe o que o monitor montar."
    );

    println!(
        "\n{:>6} {:>7} {:>8} {:>11} {:>11} {:>10} {:>11} {:>11}",
        "min", "voltas", "paradas", "dnf s/serv", "dnf c/serv", "Δ dnf", "grave s/", "grave c/"
    );
    for &min in &DURACOES_ENDURANCE {
        let voltas = voltas_da_producao(5.0, i32::from(min)) as u32;
        let d = DuracaoDeProva::constante(min);
        let paradas = d.paradas_modeladas_da_ia();
        // Voltas de serviço distribuídas uniformemente — o mesmo espaçamento que um stint de
        // ~30 min produz.
        let service: Vec<u32> = (1..=paradas)
            .map(|i| (voltas * i / (paradas + 1)).max(1))
            .collect();

        let (mut dnf_sem, mut dnf_com, mut grave_sem, mut grave_com) = (0u32, 0u32, 0u32, 0u32);
        for i in 0..AMOSTRAS {
            let car = carro_em_regime(i % CARROS + 1, i / CARROS);
            let semente = u64::from(i) * 977 + 13;
            let (_, g1, d1, _) = rodar_corrida(&car, voltas, semente, true, &[]);
            let (_, g2, d2, _) = rodar_corrida(&car, voltas, semente, true, &service);
            dnf_sem += d1;
            dnf_com += d2;
            grave_sem += g1;
            grave_com += g2;
        }
        let n = f64::from(AMOSTRAS);
        let (a, b) = (
            100.0 * f64::from(dnf_sem) / n,
            100.0 * f64::from(dnf_com) / n,
        );
        println!(
            "{min:>6} {voltas:>7} {paradas:>8} {a:>10.1}% {b:>10.1}% {:>9.1}% {:>11.2} {:>11.2}",
            b - a,
            f64::from(grave_sem) / n,
            f64::from(grave_com) / n,
        );
    }
    println!(
        "  a leitura que importa: se `Δ dnf` derruba o risco a ~zero, o serviço vira imunidade\n  \
         e a corrida longa deixa de ter consequência mecânica."
    );

    // O mesmo eixo, com o número de paradas variando na duração mais longa.
    println!("\n-- 360 min: sensibilidade ao NÚMERO de paradas de serviço --");
    let voltas = voltas_da_producao(5.0, 360) as u32;
    println!(
        "{:>9} {:>10} {:>11} {:>11}",
        "paradas", "dnf %", "graves/car", "leves/car"
    );
    for paradas in [0u32, 1, 2, 4, 8, 12] {
        let service: Vec<u32> = (1..=paradas)
            .map(|i| (voltas * i / (paradas + 1)).max(1))
            .collect();
        let (mut l, mut g, mut dn) = (0u32, 0u32, 0u32);
        for i in 0..AMOSTRAS {
            let car = carro_em_regime(i % CARROS + 1, i / CARROS);
            let (a, b, c, _) = rodar_corrida(&car, voltas, u64::from(i) * 977 + 13, true, &service);
            l += a;
            g += b;
            dn += c;
        }
        let n = f64::from(AMOSTRAS);
        println!(
            "{paradas:>9} {:>9.1}% {:>11.2} {:>11.2}",
            100.0 * f64::from(dn) / n,
            f64::from(g) / n,
            f64::from(l) / n
        );
    }
}

// ─────────────────────────────── B19 ───────────────────────────────

/// **B19** — o teto de 50 voltas de `calendar::montagem::estimate_laps`: quantas etapas batem
/// nele e quanto custa levantá-lo.
#[test]
#[ignore = "medição B19; roda com --ignored --nocapture"]
fn b19_cap_de_voltas() {
    use crate::constants::tracks::get_all_tracks;

    println!("\n===== B19 — TETO DE VOLTAS: O ANTES (50 fixo) CONTRA O DEPOIS (50 / 150) =====");
    println!(
        "  as colunas 'no teto' e 'voltas max' medem o teto ANTIGO, de 50 para toda duracao.\n  \
         Desde 12/08/2026 `calendar::montagem::teto_de_voltas` da 50 ate 60 min e 150 acima\n  \
         disso, entao 'sem teto max' e o que a prova longa passou a poder valer, ja que 150\n  \
         nao morde em nenhuma das quatro duracoes do Endurance nas pistas do catalogo."
    );
    let pistas = get_all_tracks();
    println!("  catálogo: {} pistas", pistas.len());

    // ── 1. Quantas pistas × durações batem no teto ──
    println!("\n-- fração de pistas que satura o teto, por duração --");
    println!(
        "{:>6} {:>10} {:>10} {:>12} {:>12} {:>12}",
        "min", "no teto", "% do cat.", "voltas min", "voltas máx", "sem teto máx"
    );
    for &min in &[20i32, 25, 30, 45, 60, 120, 180, 240, 360] {
        let mut no_teto = 0usize;
        let (mut menor, mut maior, mut maior_livre) = (i32::MAX, 0i32, 0i32);
        for t in pistas {
            let v = voltas_estimadas(t.comprimento_km, min, 50);
            let livre = voltas_estimadas(t.comprimento_km, min, i32::MAX);
            if v == 50 {
                no_teto += 1;
            }
            menor = menor.min(v);
            maior = maior.max(v);
            maior_livre = maior_livre.max(livre);
        }
        println!(
            "{min:>6} {no_teto:>10} {:>9.0}% {menor:>12} {maior:>12} {maior_livre:>12}",
            100.0 * no_teto as f64 / pistas.len() as f64
        );
    }
    println!("  o teto só morde de fato nas durações do Endurance; nas sprints ele é inerte.");

    // ── 2. Custo computacional por teto ──
    println!("\n-- custo computacional do modelo de quebra por teto de voltas --");
    println!(
        "{:>10} {:>12} {:>14} {:>12}",
        "teto", "voltas 360m", "ms / grade 36", "ms / etapa"
    );
    for &cap in &[50i32, 75, 100, 150, i32::MAX] {
        let voltas = voltas_estimadas(5.0, 360, cap) as u32;
        let inicio = std::time::Instant::now();
        for i in 0..CARROS {
            let car = carro_em_regime(i + 1, 0);
            let _ = rodar_corrida(&car, voltas, u64::from(i) * 977 + 13, true, &[]);
        }
        let dur = inicio.elapsed();
        let rotulo = if cap == i32::MAX {
            "sem teto".to_string()
        } else {
            cap.to_string()
        };
        println!(
            "{rotulo:>10} {voltas:>12} {:>14.2} {:>12.2}",
            dur.as_secs_f64() * 1000.0,
            dur.as_secs_f64() * 1000.0
        );
    }
    println!(
        "  o motor de corrida NÃO itera por volta (`simulation::race::motor` roda 5 segmentos),\n  \
         então levantar o teto não muda o custo da simulação interna. Quem escala linearmente\n  \
         com as voltas é o modelo de quebra ao vivo e o Monte Carlo do card."
    );

    // ── 3. Granularidade de pit ──
    println!("\n-- granularidade da janela de parada (0,35–0,65 da distância) --");
    println!(
        "{:>10} {:>12} {:>16} {:>18}",
        "teto", "voltas 360m", "voltas na janela", "% da corrida/volta"
    );
    for &cap in &[50i32, 75, 100, 150, i32::MAX] {
        let voltas = voltas_estimadas(5.0, 360, cap);
        let na_janela = ((voltas as f64) * 0.30).round() as i32;
        let rotulo = if cap == i32::MAX {
            "sem teto".to_string()
        } else {
            cap.to_string()
        };
        println!(
            "{rotulo:>10} {voltas:>12} {na_janela:>16} {:>17.2}%",
            100.0 / voltas as f64
        );
    }
}

// ─────────────────────────────── B20 ───────────────────────────────

/// **B20** — o ANTES e o DEPOIS do card de previsão. `dnf@18` é o que ele mostrava com as 18
/// voltas fixas; `dnf@real` é o que ele mostra desde 12/08/2026, lendo `CalendarEntry::voltas`.
#[test]
#[ignore = "medição B20; roda com --ignored --nocapture"]
fn b20_previsao_de_18_voltas() {
    println!("\n===== B20 — 18 VOLTAS FIXAS (ANTES) CONTRA AS VOLTAS DA ETAPA (DEPOIS) =====");
    println!(
        "  `commands::iracing::previsao_quebras` chamava `forecast_breakdown_risk` com\n  \
         `laps = 18` nos dois call sites (card do jogador, 400 amostras; tabela do campeonato,\n  \
         150). 18 é `car::wear::REF_RACE_LAPS`, a referência de DESGASTE de um sprint — nunca\n  \
         foi a duração da etapa. Hoje os dois lêem `ctx.voltas`, então a coluna 'dnf@real' É o\n  \
         card, e 'erro abs' é o que o jogador via de errado."
    );

    // Sprint e enduro, para separar o erro de duração do erro de regime.
    println!("\n-- risco previsto (18 voltas) contra o risco da etapa real --");
    println!(
        "{:>12} {:>6} {:>7} {:>10} {:>10} {:>10} {:>10}",
        "cenário", "min", "voltas", "dnf@18", "dnf@real", "erro abs", "erro rel"
    );
    let cenarios: [(&str, u16, bool); 6] = [
        ("sprint", 20, false),
        ("sprint", 45, false),
        ("enduro", 120, true),
        ("enduro", 180, true),
        ("enduro", 240, true),
        ("enduro", 360, true),
    ];
    for (rotulo, min, is_enduro) in cenarios {
        let voltas = voltas_da_producao(5.0, i32::from(min)) as u32;
        // Uma frota de 36 carros: a previsão é por carro, e o erro do card é o erro médio.
        let (mut soma18, mut somareal, mut soma_abs, mut soma_rel) = (0.0, 0.0, 0.0, 0.0);
        for i in 0..CARROS {
            let car = carro_em_regime(i + 1, 0);
            let f18 = forecast_breakdown_risk(
                &car,
                18,
                u64::from(i) * 977 + 13,
                PIT_CREW,
                TRACK,
                Weather::NEUTRAL,
                &[],
                400,
                is_enduro,
                true,
            );
            let freal = forecast_breakdown_risk(
                &car,
                voltas,
                u64::from(i) * 977 + 13,
                PIT_CREW,
                TRACK,
                Weather::NEUTRAL,
                &[],
                400,
                is_enduro,
                true,
            );
            soma18 += f18.dnf_prob;
            somareal += freal.dnf_prob;
            soma_abs += (f18.dnf_prob - freal.dnf_prob).abs();
            if freal.dnf_prob > 1e-6 {
                soma_rel += (f18.dnf_prob - freal.dnf_prob).abs() / freal.dnf_prob;
            }
        }
        let n = f64::from(CARROS);
        println!(
            "{rotulo:>12} {min:>6} {voltas:>7} {:>9.1}% {:>9.1}% {:>9.1}% {:>9.0}%",
            100.0 * soma18 / n,
            100.0 * somareal / n,
            100.0 * soma_abs / n,
            100.0 * soma_rel / n,
        );
    }

    // Sensibilidade por classe: `apply_tent` é ligado por `category_ceiling > 2`.
    println!("\n-- sensibilidade por classe (tenda de proteção ligada/desligada), 360 min --");
    println!(
        "{:>12} {:>10} {:>10} {:>10}",
        "tenda", "dnf@18", "dnf@real", "erro abs"
    );
    let voltas = voltas_da_producao(5.0, 360) as u32;
    for tent in [false, true] {
        let (mut s18, mut sreal, mut sabs) = (0.0, 0.0, 0.0);
        for i in 0..CARROS {
            let car = carro_em_regime(i + 1, 0);
            let f18 = forecast_breakdown_risk(
                &car,
                18,
                u64::from(i) * 977 + 13,
                PIT_CREW,
                TRACK,
                Weather::NEUTRAL,
                &[],
                400,
                true,
                tent,
            );
            let freal = forecast_breakdown_risk(
                &car,
                voltas,
                u64::from(i) * 977 + 13,
                PIT_CREW,
                TRACK,
                Weather::NEUTRAL,
                &[],
                400,
                true,
                tent,
            );
            s18 += f18.dnf_prob;
            sreal += freal.dnf_prob;
            sabs += (f18.dnf_prob - freal.dnf_prob).abs();
        }
        let n = f64::from(CARROS);
        println!(
            "{:>12} {:>9.1}% {:>9.1}% {:>9.1}%",
            if tent { "ligada" } else { "desligada" },
            100.0 * s18 / n,
            100.0 * sreal / n,
            100.0 * sabs / n
        );
    }

    println!(
        "\n-- COMO LER, DEPOIS DA CORREÇÃO --\n  \
         o card agora É a coluna 'dnf@real': `previsao_quebras::voltas_do_forecast` lê\n  \
         `CalendarEntry::voltas`, que é a MESMA contagem com que a corrida roda. O erro do\n  \
         card deixa de ser calibrado e passa a ser zero por construção — não há duas réguas.\n  \
         A física de quebra não foi tocada: `forecast_breakdown_risk` é o mesmo Monte Carlo\n  \
         sobre o mesmo modelo, só recebendo a duração certa.\n  \
         Os limiares de APRESENTAÇÃO continuam os de sempre (baixo < 5%, médio < 12%) porque\n  \
         eles separam bem os cenários novos: sprint de 20 min cai em 'baixo' (3,8%, e antes\n  \
         acendia 'alto' com 16,2%) e o Endurance sobe para 'médio' (8,3–10,7%, e antes ficava\n  \
         em 'baixo' com 2,5%). Trocar número de limiar aqui seria calibrar em cima de um erro\n  \
         que acabou de sumir."
    );
}
