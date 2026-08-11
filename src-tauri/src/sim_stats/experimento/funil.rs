//! Relatório do FUNIL: textura do Rookie, tiers alcançados, deflação da grade e preenchimento de emergência.

use crate::sim_stats::*;

pub(super) fn imprimir(t: &Totals, runs: usize, seasons: usize) {
    // ── TEXTURA DE NOMES DO ROOKIE ──────────────────────────────────────────
    if t.rookie_season_count > 0 {
        let sc = t.rookie_season_count as f64;
        println!("\n╔══════════════════════════════════════════════════════════════╗");
        println!("║  TEXTURA DE NOMES DO ROOKIE (por temporada, exclui a 1ª)      ║");
        println!("╚══════════════════════════════════════════════════════════════╝");
        println!("  Temporadas-rookie observadas:  {}", t.rookie_season_count);
        println!(
            "  Grid médio do Rookie:          {:.1} assentos",
            t.rookie_obs as f64 / sc
        );
        println!("\n  composição média do grid por temporada:");
        println!(
            "    Estreias NOVAS (nome inédito):       {:>5.1}  ({:.1}%)",
            t.rookie_fresh as f64 / sc,
            pct(t.rookie_fresh, t.rookie_obs)
        );
        println!(
            "    Retidos (mesmo piloto do ano ant.):  {:>5.1}  ({:.1}%)",
            t.rookie_retained as f64 / sc,
            pct(t.rookie_retained, t.rookie_obs)
        );
        println!(
            "    Conhecidos retornando (ag. livre):   {:>5.1}  ({:.1}%)",
            t.rookie_returning as f64 / sc,
            pct(t.rookie_returning, t.rookie_obs)
        );
        if t.rookie_age_n > 0 {
            println!(
                "  Idade média no Rookie:         {:.1} anos",
                t.rookie_age_sum as f64 / t.rookie_age_n as f64
            );
        }
    }

    // ── FUNIL DE CARREIRA ───────────────────────────────────────────────────
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║  FUNIL DE CARREIRA — pilotos que começaram no Rookie          ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!(
        "  Cohort (estrearam no tier 0 / Rookie): {}",
        t.rookie_cohort
    );
    println!("\n  Tier alcançado          | % do cohort | tempo médio p/ chegar");
    println!("  ------------------------+-------------+----------------------");
    for tier in 1..=6usize {
        let reached = t.reached_tier[tier];
        let avg_time = if t.time_to_tier_n[tier] > 0 {
            format!(
                "{:.1} temporadas",
                t.time_to_tier_sum[tier] as f64 / t.time_to_tier_n[tier] as f64
            )
        } else {
            "—".to_string()
        };
        println!(
            "  {:<2} {:<20} | {:>9.1}% | {}",
            tier,
            tier_label(tier as u8),
            pct(reached, t.rookie_cohort),
            avg_time
        );
    }

    println!("\n  ■ Impacto do talento (PICO de habilidade na carreira → topo atingido)");
    println!("    pico de skill    |   n  | tier médio | % que chega ao topo (5+)");
    println!("    -----------------+------+------------+-------------------------");
    for band in ["Elite (65+)", "Bom (57-65)", "Comum (<57)"] {
        if let Some(a) = t.skill_band.get(band) {
            if a[0] == 0 {
                continue;
            }
            println!(
                "    {:<16} | {:>4} | {:>10.2} | {:.1}%",
                band,
                a[0],
                a[1] as f64 / a[0] as f64,
                pct(a[2], a[0])
            );
        }
    }
    println!(
        "\n  (Nota: corridas longas dão mais tempo de carreira — rode com IRACER_MC_SEASONS alto)"
    );

    // ── KPI ANTI-DEFLAÇÃO — o melhor carro fica com o melhor piloto? ──────────
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║  DEFLAÇÃO DA GRADE — correlação carro↔skill por tier          ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!("  (r→+1 = melhor carro com melhor piloto; r→0 = grade deflacionada/mal alocada)");
    println!("\n  tier                  |    n   | skill médio | corr r(carro,skill)");
    println!("  ----------------------+--------+-------------+--------------------");
    for (tier, m) in &t.grid_corr_by_tier {
        let n = m[0];
        if n < 2.0 {
            continue;
        }
        let mean_skill = m[2] / n;
        let cov = m[5] - m[1] * m[2] / n;
        let var_car = m[3] - m[1] * m[1] / n;
        let var_skill = m[4] - m[2] * m[2] / n;
        let denom = (var_car * var_skill).max(0.0).sqrt();
        let r = if denom > 1e-9 { cov / denom } else { 0.0 };
        println!(
            "  {:<2} {:<18} | {:>6} | {:>11.2} | {:>+.3}",
            tier,
            tier_label(*tier),
            n as u64,
            mean_skill,
            r
        );
    }

    let em_promo =
        crate::market::pipeline::EMERGENCY_PROMOTIONS.load(std::sync::atomic::Ordering::Relaxed);
    let em_rookie =
        crate::market::pipeline::EMERGENCY_ROOKIES.load(std::sync::atomic::Ordering::Relaxed);
    println!("\n[ PREENCHIMENTO DE EMERGENCIA (escassez na escada fechada) ]");
    println!("  Promocoes de emergencia (ignora merito): {em_promo}");
    println!("  Rookies de emergencia (sem feeder):      {em_rookie}");
    println!(
        "  Total de disparos em {} ciclos: {}",
        runs * seasons,
        em_promo + em_rookie
    );

    // PRA ONDE: tier da vaga que a emergência preencheu (onde a escassez bate);
    // DE ONDE: tier do feeder de onde o piloto foi promovido.
    // O estático guarda o CONTADOR por par de tiers, não a lista de eventos.
    let paths: BTreeMap<(u8, u8), u64> = crate::market::pipeline::EMERGENCY_PROMO_PATHS
        .lock()
        .map(|p| p.clone())
        .unwrap_or_default();
    let total_paths: u64 = paths.values().sum();
    if total_paths > 0 {
        let mut by_to: BTreeMap<u8, u64> = BTreeMap::new();
        let mut gaps: BTreeMap<i32, u64> = BTreeMap::new(); // (to_tier - from_tier)
        for ((from, to), n) in &paths {
            *by_to.entry(*to).or_insert(0) += n;
            *gaps.entry(*to as i32 - *from as i32).or_insert(0) += n;
        }
        println!("\n  PRA ONDE vai a promoção de emergência (tier da vaga):");
        for (to, n) in &by_to {
            println!(
                "    → {:<14} {:>4}  ({:.1}%)",
                tier_label(*to),
                n,
                pct(*n, total_paths)
            );
        }
        println!("  DE ONDE vem o piloto (salto de tiers feeder→vaga):");
        for (gap, n) in &gaps {
            println!(
                "    {} tier(s) abaixo: {:>4}  ({:.1}%)",
                gap,
                n,
                pct(*n, total_paths)
            );
        }
    }

    let retire_total: u64 = t.retire_by_tier.iter().sum();
    if retire_total > 0 {
        println!("\n  DE ONDE abrem as vagas — aposentadorias por tier:");
        for tier in 0..=6usize {
            let n = t.retire_by_tier[tier];
            if n == 0 {
                continue;
            }
            println!(
                "    {:<14} {:>5}  ({:.1}%)",
                tier_label(tier as u8),
                n,
                pct(n, retire_total)
            );
        }
    }
}
