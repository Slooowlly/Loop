//! MEDIÇÃO da taxa de quebra por corrida — quantas o rádio vai ter que falar.
//!
//! Não é teste de comportamento: é a régua que decide a redação. A pergunta que ela responde é
//! "quantas vezes por corrida o engenheiro abre o canal por causa disto", e a resposta muda o
//! projeto — um evento que sai uma vez é notícia, um que sai vinte é tagarelice.
//!
//! Roda com `cargo test --lib mede_quebras_por_corrida -- --nocapture --ignored`.

use crate::car::breakdown::*;
use crate::car::Car;

/// Um grid sintético do tamanho do nosso, corrido N vezes, contando os desfechos.
#[test]
#[ignore = "medição, não asserção — rode com --ignored --nocapture"]
fn mede_quebras_por_corrida() {
    const CORRIDAS: u32 = 300;
    const CARROS: u32 = 24;
    const VOLTAS: u32 = 18;

    let mut leve = 0u32;
    let mut grave = 0u32;
    let mut dnf = 0u32;
    let mut corridas_com_3_ou_mais_dnf = 0u32;
    let mut pior_dnf = 0u32;
    let mut pior_total = 0u32;

    for corrida in 0..CORRIDAS {
        let mut dir = BreakdownDirector::new();
        for c in 1..=CARROS {
            // Desgaste de entrada em fração da vida NOMINAL (1,0 = fim da vida, 1,2 = a
            // parede). UNIFORME em [0, 1): é o retrato de uma frota em regime, onde cada peça
            // está num ponto qualquer da própria vida e a equipe troca quando ela acaba.
            //
            // Esta distribuição é SUPOSTA, não lida de um save. É a maior fonte de erro deste
            // número, e é por isso que ele serve para escolher redação e não para calibrar o
            // modelo de quebra.
            let mut car = Car::uniform(5);
            for (i, peca) in car.parts.iter_mut().enumerate() {
                peca.wear =
                    f64::from((u32::from(c) * 37 + i as u32 * 61 + corrida * 7) % 100) / 100.0;
            }
            let semente = u64::from(corrida) * 1_000 + u64::from(c);
            dir.add_car(
                c,
                LiveBreakdown::new(&car, semente, 60.0, (0.4, 0.3, 0.3)).with_tent(true),
                Vec::new(),
            );
        }
        let (mut l, mut g, mut d) = (0u32, 0u32, 0u32);
        for volta in 1..=VOLTAS {
            let progresso = f64::from(volta) / f64::from(VOLTAS);
            for c in 1..=CARROS {
                for ev in dir.on_lap_at(c, volta, Weather::NEUTRAL, progresso) {
                    match ev.severity {
                        Severity::Light => l += 1,
                        Severity::Heavy => g += 1,
                        Severity::Dnf => d += 1,
                    }
                }
            }
        }
        leve += l;
        grave += g;
        dnf += d;
        if d >= 3 {
            corridas_com_3_ou_mais_dnf += 1;
        }
        pior_dnf = pior_dnf.max(d);
        pior_total = pior_total.max(l + g + d);
    }

    let n = f64::from(CORRIDAS);
    println!(
        "\n── quebras por corrida ({CORRIDAS} corridas × {CARROS} carros × {VOLTAS} voltas) ──"
    );
    println!("  leves      {:>6.2} por corrida", f64::from(leve) / n);
    println!("  graves     {:>6.2} por corrida", f64::from(grave) / n);
    println!("  abandonos  {:>6.2} por corrida", f64::from(dnf) / n);
    println!(
        "  TOTAL      {:>6.2} por corrida",
        f64::from(leve + grave + dnf) / n
    );
    println!(
        "  corridas com 3+ abandonos: {:.0}%  (pior corrida: {pior_dnf} abandonos, \
         {pior_total} falas)",
        100.0 * f64::from(corridas_com_3_ou_mais_dnf) / n,
    );
}
