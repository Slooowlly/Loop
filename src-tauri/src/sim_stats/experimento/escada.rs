//! Relatório da ESCADA: promoção/rebaixamento, soft landing, snowball, títulos, reputação, moral, rivalidade e salários.

use crate::sim_stats::*;

pub(super) fn imprimir(t: &Totals) {
    println!("\n■ MOVIMENTOS DE EQUIPE (promoção/rebaixamento entre categorias)");
    println!(
        "    Promovidas:  {}   |   Rebaixadas:  {}",
        t.team_promoted, t.team_relegated
    );

    println!(
        "\n■ SOFT LANDING — sobrevivência do promovido (Ideia 1; flag IRACER_PROMO_SOFT_LANDING)"
    );
    println!("  Onde o carro do promovido aterrissa no campo de destino (pós-promoção):");
    if t.promo_landing_n > 0 {
        println!(
            "    Gap médio p/ o lanterna do campo: {:+.2} pts de carro   (>0 acima do pior; <0 ISOLADO abaixo)",
            t.promo_landing_gap_sum / t.promo_landing_n as f64
        );
        let n = t.promo_landing_n;
        println!(
            "    Aterrissou como PIOR do campo (isolado):  {:.1}%   ({} de {})",
            pct(t.promo_landing_rank_worst, n),
            t.promo_landing_rank_worst,
            n
        );
        println!(
            "    Logo acima do lanterna (2º/3º pior):      {:.1}%   ({} de {})",
            pct(t.promo_landing_rank_near, n),
            t.promo_landing_rank_near,
            n
        );
        println!(
            "    Meio de tabela ou melhor:                 {:.1}%   ({} de {})",
            pct(t.promo_landing_rank_mid, n),
            t.promo_landing_rank_mid,
            n
        );
    } else {
        println!("    (sem promoções observadas)");
    }
    println!("  Bounce-down (subiu e caiu logo — o 'vai não vai'):");
    if t.promo_events_obs1 > 0 {
        println!(
            "    Rebaixada já na temporada seguinte (S+1): {:.1}%   ({} de {})",
            pct(t.promo_bounce_1, t.promo_events_obs1),
            t.promo_bounce_1,
            t.promo_events_obs1
        );
    }
    if t.promo_events_obs2 > 0 {
        println!(
            "    Rebaixada em ≤2 temporadas (S+1 ou S+2):  {:.1}%   ({} de {})",
            pct(t.promo_bounce_2, t.promo_events_obs2),
            t.promo_bounce_2,
            t.promo_events_obs2
        );
    }
    if t.releg_events_obs2 > 0 {
        println!(
            "  Ricochete do rebaixado (caiu e voltou a subir em ≤2): {:.1}%   ({} de {})",
            pct(t.releg_bounce_back_2, t.releg_events_obs2),
            t.releg_bounce_back_2,
            t.releg_events_obs2
        );
    }

    println!("\n■ SNOWBALL — cadeia de 'sobe-e-vence' por equipe (promoções em temporadas");
    println!("  e tiers consecutivos; 1 = campeã 1x, 3+ = sobe a escada ganhando todo ano)");
    println!("    comprimento | nº de equipes");
    println!("    ------------+--------------");
    for (len, n) in &t.ladder_chain_hist {
        println!("    {:<11} | {}", len, n);
    }
    println!("    Maior cadeia observada (qualquer run): {}", t.max_ladder_chain);
    {
        let total: u64 = t.rookie_champ_names.values().sum();
        let mut ranked: Vec<(&String, &u64)> = t.rookie_champ_names.iter().collect();
        ranked.sort_by(|a, b| b.1.cmp(a.1));
        println!("\n  IDENTIDADE — campeões da ROOKIE (nome, nº de títulos, % do total={total}):");
        for (name, n) in ranked.iter().take(10) {
            let pct = if total > 0 { 100.0 * **n as f64 / total as f64 } else { 0.0 };
            println!("    {:<28} | {:>3} | {:>4.1}%", name, n, pct);
        }
        let mut climbers: Vec<(&String, &u64)> = t.climber_names.iter().collect();
        climbers.sort_by(|a, b| b.1.cmp(a.1));
        if !climbers.is_empty() {
            println!("  IDENTIDADE — equipes que sobem 2+ tiers seguidos (nome, nº):");
            for (name, n) in climbers.iter().take(10) {
                println!("    {:<28} | {:>3}", name, n);
            }
        }
    }

    if t.title_runs > 0 {
        println!("\n■ CONCENTRAÇÃO DE TÍTULOS DE CONSTRUTORES (média entre runs)");
        println!(
            "    Fatia da equipe mais vitoriosa: {:.1}% dos títulos",
            100.0 * t.title_top_share_sum / t.title_runs as f64
        );
        println!(
            "    Nº de equipes que ganharam ≥1 título (por run): {:.1}",
            t.title_teams_with_any_sum / t.title_runs as f64
        );
    }

    if t.premium_class_count > 0 {
        println!("\n■ DINASTIAS por CLASSE premium (Production/GT3/GT4/LMP2/Endurance)");
        println!(
            "    Vencedores únicos por classe (média):  {:.2}   (sem dinastia ~6; alvo ~3)",
            t.premium_unique_sum / t.premium_class_count as f64
        );
        println!(
            "    Fatia da equipe TOP da classe (média): {:.1}%   (alvo alto mas < ~55%)",
            100.0 * t.premium_top_share_sum / t.premium_class_count as f64
        );
    }

    if !t.rep_by_tier.is_empty() {
        println!("\n■ REPUTAÇÃO VIVA por tier (semente plana ~±3 → deve separar topo/fundo)");
        println!("    tier | média | desv.pad | mínimo | máximo | nº");
        println!("    -----+-------+----------+--------+--------+------");
        for (tier, r) in &t.rep_by_tier {
            let n = r[4];
            if n <= 0.0 {
                continue;
            }
            let mean = r[0] / n;
            let var = (r[1] / n - mean * mean).max(0.0);
            println!(
                "     {:>3} | {:>5.1} | {:>8.1} | {:>6.1} | {:>6.1} | {:>5.0}",
                tier,
                mean,
                var.sqrt(),
                r[2],
                r[3],
                n
            );
        }
    }

    if t.morale_dist[4] > 0.0 {
        let md = t.morale_dist;
        let mean = md[0] / md[4];
        let var = (md[1] / md[4] - mean * mean).max(0.0);
        if !t.focus_dist.is_empty() {
            let total: u64 = t.focus_dist.values().sum();
            println!("\n■ FOCO DA EQUIPE (deve espalhar entre as 6 fases, não travar)");
            let mut rows: Vec<(&String, &u64)> = t.focus_dist.iter().collect();
            rows.sort_by(|a, b| b.1.cmp(a.1));
            for (foco, n) in rows {
                let pct = if total > 0 {
                    100.0 * *n as f64 / total as f64
                } else {
                    0.0
                };
                println!("    {:<20} | {:>5} | {:>4.1}%", foco, n, pct);
            }
        }

        if t.bond_pairs > 0 {
            let avg = t.bond_tenure_sum / t.bond_pairs as f64;
            let pct_ge3 = 100.0 * t.bond_ge3 as f64 / t.bond_pairs as f64;
            let pct_ge4 = 100.0 * t.bond_ge4 as f64 / t.bond_pairs as f64;
            println!("\n■ VÍNCULO piloto-equipe (duplas de era SEM congelar o mercado)");
            println!(
                "    tenure médio {:.2} temporadas | ≥3 juntos {:.1}% | ≥4 (era) {:.1}% | máx {} | pares {}",
                avg, pct_ge3, pct_ge4, t.bond_max_tenure, t.bond_pairs
            );
        }

        println!("\n■ MORAL VIVA (travada em 1.0 = morta → deve variar por forma/treta)");
        println!(
            "    média {:.3} | desv.pad {:.3} | mínimo {:.2} | máximo {:.2} | nº {:.0}",
            mean,
            var.sqrt(),
            md[2],
            md[3],
            md[4]
        );
    }

    // ── RIVALIDADE ENTRE EQUIPES (Fase 2: poucas e quentes, 4 fontes) ───────
    if t.tr_runs > 0 {
        println!("\n■ RIVALIDADE ENTRE EQUIPES (vivas ≥20; deve existir, ser POUCA e quente)");
        let avg_count = t.tr_count_sum as f64 / t.tr_runs as f64;
        let avg_perceived = if t.tr_count_sum > 0 {
            t.tr_perceived_sum / t.tr_count_sum as f64
        } else {
            0.0
        };
        println!(
            "    vivas/run {:.1} | percebida média {:.1} | máx {:.1}",
            avg_count, avg_perceived, t.tr_perceived_max
        );
        if t.tr_by_source.is_empty() {
            println!("    (nenhuma rivalidade viva — nenhuma fonte disparou)");
        } else {
            print!("    por fonte:");
            for (src, n) in &t.tr_by_source {
                print!("  {src}={n}");
            }
            println!();
        }
    }

    println!("\n■ SALÁRIOS por tier (anual, contratos ativos da IA)");
    println!("    tier | média        | mínimo      | máximo      | nº");
    println!("    -----+--------------+-------------+-------------+-----");
    for (tier, a) in &t.salary_by_tier {
        if a[1] == 0.0 {
            continue;
        }
        println!(
            "    {:<4} | {:>12.0} | {:>11.0} | {:>11.0} | {:.0}",
            tier,
            a[0] / a[1],
            a[2],
            a[3],
            a[1]
        );
    }
}
