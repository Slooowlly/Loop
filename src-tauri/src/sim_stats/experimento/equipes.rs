//! Relatório das EQUIPES: saúde financeira, recuperação, desfecho de colapso e atributos.

use crate::sim_stats::*;

pub(super) fn imprimir(t: &Totals) {
    // ── EQUIPES ────────────────────────────────────────────────────────────────
    println!("\n╔══════════════════════════════════════════════════════════════╗");
    println!(
        "║  EQUIPES ({} equipe-temporada observadas)",
        t.team_seasons
    );
    println!("╚══════════════════════════════════════════════════════════════╝");

    println!("\n■ SAÚDE FINANCEIRA");
    // Ordenar estados do mais saudável ao pior
    let state_order = [
        "elite",
        "healthy",
        "stable",
        "pressured",
        "crisis",
        "collapse",
    ];
    for st in state_order {
        if let Some(c) = t.fin_state.get(st) {
            println!("    {:<10} {:>6}  ({:.1}%)", st, c, pct(*c, t.team_seasons));
        }
    }
    for (st, c) in &t.fin_state {
        if !state_order.contains(&st.as_str()) {
            println!("    {:<10} {:>6}  ({:.1}%)", st, c, pct(*c, t.team_seasons));
        }
    }
    println!(
        "  Equipes com caixa negativo ou dívida: {:.1}%",
        pct(t.team_insolvent, t.team_seasons)
    );
    if t.team_seasons > 0 {
        println!(
            "  Caixa médio: {:>12.0}   |   Dívida média: {:>12.0}",
            t.cash_sum / t.team_seasons as f64,
            t.debt_sum / t.team_seasons as f64
        );
    }

    println!("\n■ RECUPERAÇÃO (trajetória individual: equipes que colapsaram ao menos 1x)");
    println!(
        "  Equipes que entraram em colapso: {}",
        t.teams_ever_collapse
    );
    if t.teams_ever_collapse > 0 {
        println!(
            "    → RECUPERARAM (chegaram a 'stable'+ depois):  {:.1}%",
            pct(t.teams_recovered, t.teams_ever_collapse)
        );
        println!(
            "    → saíram do colapso (qualquer estado melhor):  {:.1}%",
            pct(t.teams_escaped, t.teams_ever_collapse)
        );
        println!(
            "    → PRESAS (terminaram a simulação em colapso):  {:.1}%",
            pct(t.teams_stuck, t.teams_ever_collapse)
        );
        println!(
            "  Temporadas médias em colapso por equipe: {:.1}",
            t.collapse_seasons_sum as f64 / t.teams_ever_collapse as f64
        );
        if t.recover_time_n > 0 {
            println!(
                "  Tempo médio para recuperar (das que recuperaram): {:.1} temporadas",
                t.recover_time_sum as f64 / t.recover_time_n as f64
            );
        }
    }

    let episodes_resolved = t.episodes_self_rescued + t.episodes_sold;
    println!(
        "\n■ DESFECHO DOS EPISÓDIOS DE COLAPSO (resolvidos: {})",
        episodes_resolved
    );
    println!(
        "    Salvaram-se sozinhas no all-in (SEM venda): {}  ({:.1}%)",
        t.episodes_self_rescued,
        pct(t.episodes_self_rescued, episodes_resolved)
    );
    println!(
        "    Precisaram ser VENDIDAS (nova diretoria):   {}  ({:.1}%)",
        t.episodes_sold,
        pct(t.episodes_sold, episodes_resolved)
    );
    println!(
        "    Eventos de venda gravados na ficha:         {}  (deve bater com vendidas)",
        t.ownership_events_recorded
    );

    if t.team_seasons > 0 {
        let n = t.team_seasons as f64;
        println!("\n■ ATRIBUTOS MÉDIOS DE EQUIPE (0-100)");
        println!("    Instalações:    {:.1}", t.team_attr_sum[0] / n);
        println!("    Engenharia:     {:.1}", t.team_attr_sum[1] / n);
        println!("    Reputação:      {:.1}", t.team_attr_sum[2] / n);
        println!("    Confiabilidade: {:.1}", t.team_attr_sum[4] / n);
        println!("    Moral (mult.):  {:.2}", t.team_attr_sum[3] / n);
    }
}
