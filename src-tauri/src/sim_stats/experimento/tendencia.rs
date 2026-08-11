//! **A tabela POR TEMPORADA** — a régua que o agregado do Monte Carlo não tem.
//!
//! ## Por que esta seção existe
//!
//! O resto do relatório soma todas as temporadas de todas as runs e imprime uma média. Isso
//! responde "como é este mundo", e é uma pergunta legítima. Não responde a pergunta de
//! calibração, que é outra: **este mundo é ESTÁVEL?**
//!
//! Um mundo que lesiona 1,2% dos pilotos na primeira temporada e 2,8% na oitava tem a mesma
//! média de um que lesiona 2,0% em todas — e são mundos opostos. O primeiro está derretendo:
//! cada temporada tira mais gente do que a anterior, e em vinte anos de carreira jogada o
//! jogador vê um grid que se esvazia. O agregado não tem como mostrar isso.
//!
//! ## O ruído, e por que ele obriga a olhar tendência
//!
//! Execuções idênticas do Monte Carlo variam entre si em torno de 8–10%. Com esse chão, comparar
//! o agregado de duas rodadas — antes e depois de uma mudança de constante — **não distingue** o
//! efeito da mudança do sorteio da rodada. É a armadilha clássica: mede-se, o número muda, e
//! atribui-se a mudança ao que se acabou de fazer.
//!
//! Uma tendência ao longo do índice de temporada é mais robusta que uma diferença de agregado,
//! por uma razão simples: ela é uma medida INTERNA à rodada. Se a taxa sobe monotonicamente da
//! temporada 1 à 8 na rodada A e faz o mesmo na rodada B, o ruído entre rodadas não apagou o
//! sinal — ele desloca a curva inteira para cima ou para baixo, não inverte a inclinação.
//!
//! Por isso a tabela imprime, para cada índice de temporada: a média entre runs, o desvio entre
//! runs (a régua do ruído naquele ponto) e a população que serviu de denominador. E fecha com a
//! variação da primeira para a última temporada, ao lado do desvio típico — **é a comparação
//! desses dois números, e não a variação sozinha, que autoriza chamar algo de tendência.**

use super::*;

pub(super) fn imprimir(t: &Totals) {
    if t.taxas_por_temporada.is_empty() {
        return;
    }

    println!("\n┌─────────────────────────────────────────────────────────────┐");
    println!("│ TENDÊNCIA POR TEMPORADA (média entre runs ± desvio entre runs)");
    println!("└─────────────────────────────────────────────────────────────┘");
    println!(
        "{:>4}  {:>15}  {:>15}  {:>15}  {:>15}  {:>9}",
        "T", "lesão %", "aposent. %", "sobe %", "desce %", "ativos"
    );

    let celula = |v: &[f64]| format!("{:>7.2} ±{:<6.2}", mean(v), stddev(v));

    for (indice, taxas) in &t.taxas_por_temporada {
        println!(
            "{:>4}  {}  {}  {}  {}  {:>9.0}",
            indice + 1,
            celula(&taxas.lesao),
            celula(&taxas.aposentadoria),
            celula(&taxas.sobe),
            celula(&taxas.desce),
            mean(&taxas.ativos),
        );
    }

    // ── O veredito ────────────────────────────────────────────────────────────
    //
    // A regra é deliberadamente conservadora: só chama de TENDÊNCIA o que anda mais do que o
    // dobro do ruído típico daquela série. Abaixo disso a variação é indistinguível do sorteio,
    // e dizer "subiu" seria exatamente o erro que esta seção existe para evitar.
    let Some((_, primeira)) = t.taxas_por_temporada.iter().next() else {
        return;
    };
    let Some((_, ultima)) = t.taxas_por_temporada.iter().next_back() else {
        return;
    };
    if t.taxas_por_temporada.len() < 3 {
        println!(
            "\n(poucas temporadas para veredito de tendência — rode com IRACER_MC_SEASONS maior)"
        );
        return;
    }

    println!(
        "\n{:>16}  {:>10}  {:>10}  {:>10}  {}",
        "série", "1ª", "última", "Δ", "veredito"
    );
    let series: [(&str, &Vec<f64>, &Vec<f64>); 4] = [
        ("lesão %", &primeira.lesao, &ultima.lesao),
        (
            "aposentadoria %",
            &primeira.aposentadoria,
            &ultima.aposentadoria,
        ),
        ("sobe %", &primeira.sobe, &ultima.sobe),
        ("ativos", &primeira.ativos, &ultima.ativos),
    ];
    for (nome, ini, fim) in series {
        let (a, b) = (mean(ini), mean(fim));
        // A régua é o MAIOR dos dois desvios: usar o menor faria a ponta mais quieta da série
        // autorizar um veredito que a ponta ruidosa não sustenta.
        let ruido = stddev(ini).max(stddev(fim));
        let delta = b - a;
        let veredito = if ruido <= f64::EPSILON {
            if delta.abs() > f64::EPSILON {
                "TENDÊNCIA (sem ruído medido)"
            } else {
                "estável"
            }
        } else if delta.abs() > 2.0 * ruido {
            if delta > 0.0 {
                "TENDÊNCIA ↑"
            } else {
                "TENDÊNCIA ↓"
            }
        } else {
            "dentro do ruído"
        };
        println!("{nome:>16}  {a:>10.2}  {b:>10.2}  {delta:>+10.2}  {veredito}");
    }
    println!(
        "\n(\"dentro do ruído\" = |Δ| ≤ 2× o desvio entre runs. Conclusão de calibração tirada de\n\
         uma linha assim é indistinguível do sorteio — rode mais runs antes de acreditar.)"
    );
}
