//! O experimento em si: a run Monte Carlo que cria carreiras, avança as
//! temporadas, coleta os snapshots e imprime o relatório.
//!
//! Este arquivo é só a FACHADA: `monte_carlo` continua morando aqui (o caminho
//! do teste — `sim_stats::experimento::monte_carlo` — não muda), e cada etapa
//! virou um submódulo em `experimento/`: `coleta` roda as runs e alimenta os
//! [`Totals`]; `pilotos`/`equipes`/`carros`/`escada`/`funil` imprimem as seções
//! do relatório.

use super::*;

#[path = "experimento/carros.rs"]
mod carros;
#[path = "experimento/coleta.rs"]
mod coleta;
#[path = "experimento/equipes.rs"]
mod equipes;
#[path = "experimento/escada.rs"]
mod escada;
#[path = "experimento/fama.rs"]
mod fama;
#[path = "experimento/funil.rs"]
mod funil;
#[path = "experimento/pilotos.rs"]
mod pilotos;

// ── O experimento ─────────────────────────────────────────────────────────────

#[test]
#[ignore = "harness de estatística; rode manualmente com --nocapture --ignored"]
pub(super) fn monte_carlo() {
    let runs: usize = std::env::var("IRACER_MC_RUNS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(5);
    let seasons: usize = std::env::var("IRACER_MC_SEASONS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(8);

    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!("║  MONTE CARLO — estatísticas de carreira iRacer                ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!(
        "Runs (carreiras): {runs}   Temporadas por run: {seasons}   Total ciclos: {}",
        runs * seasons
    );
    println!(
        "Threshold de estagnação: ±{STAGNATION_THRESHOLD} pts (média de {} atributos)\n",
        CORE_ATTRS.len()
    );

    let mut t = Totals::default();
    crate::market::pipeline::EMERGENCY_PROMOTIONS.store(0, std::sync::atomic::Ordering::Relaxed);
    crate::market::pipeline::EMERGENCY_ROOKIES.store(0, std::sync::atomic::Ordering::Relaxed);
    if let Ok(mut paths) = crate::market::pipeline::EMERGENCY_PROMO_PATHS.lock() {
        paths.clear();
    }
    let start = Instant::now();

    coleta::coletar(runs, seasons, start, &mut t);

    // ── Relatório ─────────────────────────────────────────────────────────────
    println!("\n┌─────────────────────────────────────────────────────────────┐");
    println!(
        "│ RESULTADO ({} pilotos-temporada observados)",
        t.driver_seasons
    );
    println!("└─────────────────────────────────────────────────────────────┘");

    pilotos::imprimir(&t);
    fama::imprimir(&t);
    equipes::imprimir(&t);
    carros::imprimir(&t);
    escada::imprimir(&t);
    funil::imprimir(&t, runs, seasons);

    println!("\nTempo total: {:.1}s\n", start.elapsed().as_secs_f64());
}
