//! Relatório dos CARROS: performance por tier, nível por categoria, peças, foco e foco vs pista.

use crate::sim_stats::*;

pub(super) fn imprimir(t: &Totals) {
    println!("\n■ PERFORMANCE DE CARRO por tier de categoria (média)");
    println!("    tier | car_performance | nº amostras");
    println!("    -----+-----------------+------------");
    for (tier, a) in &t.car_perf_by_tier {
        if a[1] == 0.0 {
            continue;
        }
        println!("    {:<4} | {:>15.1} | {:.0}", tier, a[0] / a[1], a[1]);
    }

    println!("\n■ NÍVEL DO CARRO (1–10) por categoria — Sistema de Nível do Carro");
    println!("    Alvo: a média deveria convergir perto do TETO da categoria, com spread (min<max).");
    println!("    categoria             | teto | média | min | max | nº");
    println!("    ----------------------+------+-------+-----+-----+-----");
    for (cat, a) in &t.car_level_by_category {
        if a[1] == 0.0 {
            continue;
        }
        let ceiling = crate::car::cost::category_ceiling(cat);
        println!(
            "    {:<21} | {:>4} | {:>5.1} | {:>3.0} | {:>3.0} | {:.0}",
            cat, ceiling, a[0] / a[1], a[2], a[3], a[1]
        );
    }

    // ── (1) Distribuição por peça ──
    println!("\n■ NÍVEL MÉDIO POR PEÇA (times não-spec) — onde o cérebro investe");
    println!("    peça          | nível médio | nº");
    println!("    --------------+-------------+------");
    let mut parts: Vec<(&String, &[f64; 2])> = t.part_level_by_type.iter().collect();
    parts.sort_by(|a, b| {
        let av = if a.1[1] > 0.0 { a.1[0] / a.1[1] } else { 0.0 };
        let bv = if b.1[1] > 0.0 { b.1[0] / b.1[1] } else { 0.0 };
        bv.partial_cmp(&av).unwrap_or(std::cmp::Ordering::Equal)
    });
    for (name, a) in parts {
        if a[1] == 0.0 {
            continue;
        }
        println!("    {:<13} | {:>11.2} | {:.0}", name, a[0] / a[1], a[1]);
    }

    // ── (2) Foco do carro ──
    println!("\n■ FOCO DO CARRO (times não-spec) — tendência de shape");
    let focus_total: u64 = t.shape_focus.values().sum();
    if focus_total > 0 {
        for (focus, n) in &t.shape_focus {
            println!(
                "    {:<12} {:>6} ({:.1}%)",
                focus,
                n,
                *n as f64 / focus_total as f64 * 100.0
            );
        }
    }

    // ── (2b) Distribuição por peça QUEBRADA por arquétipo — onde o foco/de-investimento aparece ──
    println!("\n■ DISTRIBUIÇÃO POR PEÇA × ARQUÉTIPO (nível médio; onde cada foco investe/larga)");
    {
        use crate::car::PartType;
        let focuses = ["balanceado", "potência", "handling", "aceleração"];
        let avg = |focus: &str, part: &str| -> Option<f64> {
            t.part_level_by_focus
                .get(&format!("{focus}|{part}"))
                .filter(|a| a[1] > 0.0)
                .map(|a| a[0] / a[1])
        };
        println!("    peça          | balanc. | potênc. | handl. | aceler.");
        println!("    --------------+---------+---------+--------+--------");
        // Ordena as peças pela dispersão entre arquétipos (as que mais separam no topo).
        let mut rows: Vec<(&'static str, [Option<f64>; 4], f64)> = PartType::ALL
            .iter()
            .map(|pt| {
                let name = pt.as_str();
                let vals = [
                    avg(focuses[0], name),
                    avg(focuses[1], name),
                    avg(focuses[2], name),
                    avg(focuses[3], name),
                ];
                let present: Vec<f64> = vals.iter().flatten().copied().collect();
                let spread = match (
                    present.iter().cloned().fold(f64::MIN, f64::max),
                    present.iter().cloned().fold(f64::MAX, f64::min),
                ) {
                    (mx, mn) if present.len() > 1 => mx - mn,
                    _ => 0.0,
                };
                (name, vals, spread)
            })
            .collect();
        rows.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
        let cell = |v: Option<f64>| match v {
            Some(x) => format!("{x:>5.1}  "),
            None => "   -   ".to_string(),
        };
        for (name, vals, _) in rows {
            println!(
                "    {:<13} | {} | {} | {} | {}",
                name,
                cell(vals[0]),
                cell(vals[1]),
                cell(vals[2]),
                cell(vals[3])
            );
        }
    }

    // ── (3) Foco vs pista: o ganho de um carro focado na pista do seu atributo ──
    println!("\n■ FOCO vs PISTA — bônus de car_performance por (foco do carro × tipo de pista)");
    println!("    Carros FORTEMENTE focados (4 peças no talo, resto no piso). Diagonal = casado.");
    println!("    ~1,6 pts de car_performance ≈ 1 nível de carro NAQUELA pista; ainda ×car_weight na corrida.");
    {
        use crate::car::sim_bridge::car_shape_weights;
        use crate::car::PartType::*;
        use crate::simulation::car_build::track_delta_from_shape;
        use crate::simulation::track_profile::get_track_simulation_data;
        let mk = |focus: &[crate::car::PartType]| -> crate::car::Car {
            let mut c = crate::car::Car::uniform(2);
            for &p in focus {
                c.set_level(p, 10);
            }
            c
        };
        let cars = [
            ("potência", mk(&[Engine, Gearbox, Cooling, Electronics])),
            ("handling", mk(&[FrontWing, RearWing, Brakes, Suspension])),
            ("aceleração", mk(&[Gearbox, Chassis, Suspension, Electronics])),
            ("balanceado", crate::car::Car::uniform(6)),
        ];
        let tracks = [
            ("power(Monza)", 93u32),
            ("handl(Ledenon)", 489u32),
            ("accel(Tsukuba)", 325u32),
        ];
        print!("    {:<12}", "carro\\pista");
        for (tname, _) in &tracks {
            print!(" | {:>14}", tname);
        }
        println!();
        for (cname, car) in &cars {
            let shape = car_shape_weights(car);
            print!("    {:<12}", cname);
            for (_, tid) in &tracks {
                let d = get_track_simulation_data(*tid);
                let tw = (d.acceleration_weight, d.power_weight, d.handling_weight);
                print!(" | {:>+14.2}", track_delta_from_shape(shape, tw));
            }
            println!();
        }
    }
}
