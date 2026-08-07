//! Relatório dos PILOTOS: lesões, evolução, desempenho, trajetória, aposentadorias e movimentos.

use crate::sim_stats::*;

pub(super) fn imprimir(t: &Totals) {
    println!("\n■ LESÕES");
    println!(
        "  % pilotos que se machucam / temporada: {:.1}%  (média/ciclo {:.1}% ± {:.1}, faixa {:.1}–{:.1})",
        pct(t.injured_drivers, t.driver_seasons),
        mean(&t.inj_rate_samples),
        stddev(&t.inj_rate_samples),
        minmax(&t.inj_rate_samples).0,
        minmax(&t.inj_rate_samples).1,
    );
    let inj_total = t.inj_leve + t.inj_moderada + t.inj_grave + t.inj_critica;
    println!("  Lesões geradas (total {inj_total}) por gravidade:");
    println!(
        "    Leve     {:>5}  ({:.1}%)",
        t.inj_leve,
        pct(t.inj_leve, inj_total)
    );
    println!(
        "    Moderada {:>5}  ({:.1}%)",
        t.inj_moderada,
        pct(t.inj_moderada, inj_total)
    );
    println!(
        "    Grave    {:>5}  ({:.1}%)",
        t.inj_grave,
        pct(t.inj_grave, inj_total)
    );
    println!(
        "    Crítica  {:>5}  ({:.1}%)",
        t.inj_critica,
        pct(t.inj_critica, inj_total)
    );

    println!("\n■ EVOLUÇÃO (sobreviventes: {} observações)", t.survivors);
    println!(
        "  SOBE    {:.1}%  (média/ciclo {:.1}% ± {:.1})",
        pct(t.sobe, t.survivors),
        mean(&t.sobe_rate_samples),
        stddev(&t.sobe_rate_samples)
    );
    println!(
        "  DESCE   {:.1}%  (média/ciclo {:.1}% ± {:.1})",
        pct(t.desce, t.survivors),
        mean(&t.desce_rate_samples),
        stddev(&t.desce_rate_samples)
    );
    println!(
        "  ESTAGNA {:.1}%  (média/ciclo {:.1}% ± {:.1})",
        pct(t.estagna, t.survivors),
        mean(&t.estagna_rate_samples),
        stddev(&t.estagna_rate_samples)
    );
    println!("\n  Por faixa etária:");
    println!("    idade   | sobe%  desce% estag% | Δ médio");
    println!("    --------+----------------------+--------");
    for (bucket, a) in &t.by_age {
        let n = a[4];
        if n == 0.0 {
            continue;
        }
        println!(
            "    {:<7} | {:>5.1} {:>5.1} {:>5.1} | {:+.2}",
            bucket,
            100.0 * a[0] / n,
            100.0 * a[1] / n,
            100.0 * a[2] / n,
            a[3] / n,
        );
    }

    println!("\n■ DESEMPENHO NA TEMPORADA (pilotos da IA que correram)");
    println!(
        "  Taxa de DNF (abandono): {:.1}%  ({} abandonos em {} largadas)",
        pct(t.total_dnfs as u64, t.total_starts.max(0) as u64),
        t.total_dnfs,
        t.total_starts
    );
    println!(
        "  Distribuição de vitórias por piloto-temporada ({} obs):",
        t.drivers_raced
    );
    println!(
        "    0 vitórias    {:>6}  ({:.1}%)",
        t.win_0,
        pct(t.win_0, t.drivers_raced)
    );
    println!(
        "    1–2 vitórias  {:>6}  ({:.1}%)",
        t.win_1_2,
        pct(t.win_1_2, t.drivers_raced)
    );
    println!(
        "    3–5 vitórias  {:>6}  ({:.1}%)",
        t.win_3_5,
        pct(t.win_3_5, t.drivers_raced)
    );
    println!(
        "    6+ vitórias   {:>6}  ({:.1}%)",
        t.win_6p,
        pct(t.win_6p, t.drivers_raced)
    );
    println!(
        "    com ≥1 pódio  {:>6}  ({:.1}%)",
        t.with_podium,
        pct(t.with_podium, t.drivers_raced)
    );
    if t.motiv_n > 0 {
        println!(
            "  Motivação média: {:.1}/100   |   pilotos em risco (<20): {:.1}%",
            t.motiv_sum / t.motiv_n as f64,
            pct(t.motiv_lt20, t.motiv_n)
        );
    }

    if !t.motiv_by_season.is_empty() {
        println!("\n■ MOTIVAÇÃO — TRAJETÓRIA POR TEMPORADA");
        println!("  A média de todas as temporadas juntas esconde catraca. Se a coluna 'média'");
        println!("  sobe monotonicamente e 'no teto' engorda, a motivação não é um eixo vivo —");
        println!("  é um crédito que só acumula.");
        println!("    temporada | média | no teto (100) | em risco (<20) |     n");
        println!("    ----------+-------+---------------+----------------+-------");
        for (season, e) in &t.motiv_by_season {
            let n = e[1];
            if n <= 0.0 {
                continue;
            }
            println!(
                "    {:>9} | {:>5.1} | {:>12.1}% | {:>13.1}% | {:>5.0}",
                season + 1,
                e[0] / n,
                e[2] / n * 100.0,
                e[3] / n * 100.0,
                n
            );
        }
    }

    if !t.free_agents_by_season.is_empty() {
        println!("\n■ AGENTES LIVRES (Ativo e sem contrato no início da temporada)");
        println!("  Perder o assento não aposenta: o piloto fica livre e disputa a janela. A");
        println!("  regra do órfão ocioso só age depois de uma temporada inteira sem correr, e");
        println!("  existe para o mundo não acumular agente livre eterno.");
        println!("    temporada | livres | idade média | overall médio");
        println!("    ----------+--------+-------------+--------------");
        for (season, e) in &t.free_agents_by_season {
            let n = e[0];
            if n <= 0.0 {
                continue;
            }
            println!(
                "    {:>9} | {:>6.0} | {:>11.1} | {:>13.1}",
                season + 1,
                n,
                e[1] / n,
                e[2] / n
            );
        }
        let total: u64 = t.free_streak_hist.values().sum();
        println!("\n  Temporadas CONSECUTIVAS como agente livre (o teste do 'eterno'):");
        println!("  Quase tudo na 1ª = o mercado recicla e a regra do órfão sobra. Cauda gorda");
        println!("  em 3+ = acúmulo real, e afrouxar a regra traz o problema de volta.");
        for (streak, n) in &t.free_streak_hist {
            let rotulo = if *streak >= 6 {
                "6+".to_string()
            } else {
                format!("{streak}ª")
            };
            println!(
                "    {:>4} | {:>5} ({:>4.1}%)",
                rotulo,
                n,
                pct(*n, total.max(1))
            );
        }
    }

    if !t.motiv_retire_by_age.is_empty() {
        let total: u64 = t.motiv_retire_by_age.values().sum();
        println!("\n■ QUEM LARGA POR DESMOTIVAÇÃO — POR IDADE ({total} pilotos)");
        println!("  Veterano desistir fecha; jovem desistir não. O ramo de desmotivação em");
        println!("  `check_retirement` compra paciência só com skill, sem termo de idade.");
        for (faixa, n) in &t.motiv_retire_by_age {
            println!(
                "    {:>6} | {:>4} ({:>4.1}%)",
                faixa,
                n,
                pct(*n, total.max(1))
            );
        }
    }

    if !t.mercado_by_motiv.is_empty() {
        println!("\n■ MOTIVAÇÃO × DECISÃO DE MERCADO (janela do offseason)");
        println!("  A ficha do piloto diz ao jogador que a desmotivação empurra pra troca.");
        println!("  Se as linhas de baixo não se distinguirem das de cima, ela está mentindo:");
        println!("  `src/market/` não lê `motivacao` em lugar nenhum.");
        println!("    motivação | trocou de equipe | desceu de tier | Δ salário médio |     n");
        println!("    ----------+------------------+----------------+-----------------+-------");
        for (band, e) in &t.mercado_by_motiv {
            let n = e[0];
            if n <= 0.0 {
                continue;
            }
            println!(
                "    {:>9} | {:>15.1}% | {:>13.1}% | {:>+14.1}% | {:>5.0}",
                motiv_band_label(*band),
                e[1] / n * 100.0,
                e[2] / n * 100.0,
                e[3] / n,
                n
            );
        }
    }

    println!(
        "\n■ TRAJETÓRIA DE CARREIRA ({} pilotos vistos em ≥2 temporadas; threshold ±{CAREER_THRESHOLD} pt)",
        t.traj_count
    );
    println!("  Comparando o 1º vs o último ano de cada piloto na simulação:");
    println!("  SOBE    {:.1}%", pct(t.traj_sobe, t.traj_count));
    println!("  DESCE   {:.1}%", pct(t.traj_desce, t.traj_count));
    println!("  ESTAGNA {:.1}%", pct(t.traj_estagna, t.traj_count));
    if t.traj_count > 0 {
        println!(
            "  Δ médio de carreira: {:+.2} pts",
            t.traj_delta_sum / t.traj_count as f64
        );
    }
    println!("\n  Por faixa etária (idade no 1º ano observado):");
    println!("    idade   | sobe%  desce% estag% | Δ médio");
    println!("    --------+----------------------+--------");
    for (bucket, a) in &t.traj_by_age {
        let n = a[4];
        if n == 0.0 {
            continue;
        }
        println!(
            "    {:<7} | {:>5.1} {:>5.1} {:>5.1} | {:+.2}",
            bucket,
            100.0 * a[0] / n,
            100.0 * a[1] / n,
            100.0 * a[2] / n,
            a[3] / n,
        );
    }

    println!("\n■ APOSENTADORIAS");
    println!(
        "  % aposenta / temporada: {:.1}%  (média/ciclo {:.1}% ± {:.1})",
        pct(t.retirements, t.driver_seasons),
        mean(&t.retire_rate_samples),
        stddev(&t.retire_rate_samples)
    );
    if t.retirements > 0 {
        println!(
            "  Idade média de aposentadoria: {:.1} anos",
            t.retire_age_sum as f64 / t.retirements as f64
        );
    }
    if t.retire_career_len_n > 0 {
        println!(
            "  Duração média de carreira observada: {:.1} temporadas",
            t.retire_career_len_sum as f64 / t.retire_career_len_n as f64
        );
    }
    println!("  Causas:");
    for (reason, count) in &t.retire_reasons {
        println!(
            "    {:<28} {:>4}  ({:.1}%)",
            reason,
            count,
            pct(*count, t.retirements)
        );
    }
    if t.motiv_retire_n > 0 {
        println!(
            "  Perfil de quem largou por MOTIVACAO: overall medio {:.1} | 'bons' (>=60): {} de {} ({:.1}%)",
            t.motiv_retire_overall_sum / t.motiv_retire_n as f64,
            t.motiv_retire_good,
            t.motiv_retire_n,
            pct(t.motiv_retire_good, t.motiv_retire_n)
        );
    }

    println!("\n■ PROMOÇÃO / REBAIXAMENTO (pilotos, % sobre pilotos-temporada)");
    println!(
        "  Promovidos (sobem c/ equipe):     {:>4}  ({:.1}%/temp)",
        t.promoted,
        pct(t.promoted, t.driver_seasons)
    );
    println!(
        "  Rebaixados (descem c/ equipe):    {:>4}  ({:.1}%/temp)",
        t.relegated,
        pct(t.relegated, t.driver_seasons)
    );
    println!(
        "  Liberados (sem licença p/ subir): {:>4}  ({:.1}%/temp)",
        t.freed_no_license,
        pct(t.freed_no_license, t.driver_seasons)
    );
}
