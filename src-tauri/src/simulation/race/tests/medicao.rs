//! Harness de MEDIÇÃO de lesões e peças — régua, não teste de regressão.
//!
//! Roda muitas corridas e IMPRIME o que a calibração atual produz, para escolher número
//! olhando resultado em vez de intuição. Nasceu da carreira em que 11,7% das largadas
//! terminavam com piloto machucado.
//!
//! ```text
//! cargo test --lib medir_lesoes_e_pecas -- --ignored --nocapture
//! ```

use rand::{rngs::StdRng, SeedableRng};
use std::collections::HashMap;

use crate::car::{wear::wear_per_race, Car, PartType};
use crate::models::enums::{InjuryType, WeatherCondition};
use crate::simulation::catalog::IncidentCatalog;
use crate::simulation::incidents::{IncidentResult, IncidentSeverity, IncidentType};
use crate::simulation::injuries::generate_injury_from_incident;
use crate::simulation::qualifying::simulate_qualifying;
use crate::simulation::race::simulate_race_with_breakdowns;

use super::{build_grid, sample_context_with_incidents};

const TEMPORADAS: u64 = 30;
const ETAPAS: u64 = 20;
/// Espelha `car::crash::CONTACT_WEAR_PER_HIT` para marcar a linha "atual" na varredura.
const CONTATO_ATUAL: f64 = crate::car::crash::CONTACT_WEAR_PER_HIT;
/// Categoria de referência para precificar peça (o grid de teste roda gt4/gt3).
const CATEGORIA: &str = "gt3";

/// Um contato de disputa: colisão entre dois carros que não escalou.
fn e_contato(inc: &IncidentResult) -> bool {
    inc.is_two_car_incident
        && inc.incident_type == IncidentType::Collision
        && inc.severity == IncidentSeverity::Minor
}

/// Rebobina o multiplicador de risco para o que os call sites cravavam ANTES: contato de
/// disputa `1.0` (→ 50% de lesão) e manifestação de dano latente `1.5`/`1.0` (→ 37,5%/25%).
/// Permite medir o MESMO conjunto de incidentes nas duas calibrações.
fn irm_antigo(inc: &IncidentResult) -> f64 {
    if e_contato(inc) {
        return 1.0;
    }
    if inc.damage_origin_segment.is_some() {
        return if inc.is_dnf { 1.5 } else { 1.0 };
    }
    inc.injury_risk_multiplier
}

/// Quanto tempo de afastamento resta. Reproduz a supressão real: `process_new_injuries` PULA
/// quem já está machucado, então uma lesão longa esconde as próximas. Sem isso a medição
/// superestima a taxa.
type Afastados = HashMap<String, i32>;

fn passar_uma_corrida(afastados: &mut Afastados) {
    afastados.retain(|_, restantes| {
        *restantes -= 1;
        *restantes > 0
    });
}

#[test]
#[ignore = "harness de medição — roda com --ignored --nocapture"]
fn medir_lesoes_e_pecas() {
    let grid = build_grid();
    let ctx = sample_context_with_incidents(30, WeatherCondition::Dry);

    let mut largadas = 0u64;
    let mut contatos = 0u64;
    let (mut leves, mut moderadas, mut graves) = (0u64, 0u64, 0u64);
    let mut lesoes_antigas = 0u64;
    let mut afastados: Afastados = HashMap::new();
    let mut afastados_antigos: Afastados = HashMap::new();

    // Um carro por equipe: desgaste normal de corrida + o castigo dos contatos por cima.
    let mut carros: HashMap<String, Car> = grid
        .iter()
        .map(|d| (d.team_id.clone(), Car::uniform(5)))
        .collect();
    let mut destruidas_por_contato = 0u64;
    let mut desgaste_de_contato = 0.0f64;
    let mut trocas_por_fim_de_vida = 0u64;
    let mut custo_de_contato = 0.0f64;
    let mut custo_de_manutencao = 0.0f64;
    let mut pior_rodada = 0.0f64;
    // Fluxo de contatos por equipe, corrida a corrida — guardado para a varredura no fim.
    let mut historico_de_pancadas: Vec<HashMap<String, u32>> = Vec::new();

    for corrida in 0..(TEMPORADAS * ETAPAS) {
        let mut rng = StdRng::seed_from_u64(corrida);
        let qualifying = simulate_qualifying(&grid, &ctx, &mut rng);
        let result = simulate_race_with_breakdowns(
            &grid,
            &qualifying,
            &ctx,
            &IncidentCatalog::empty(),
            false,
            None,
            &mut rng,
        );

        passar_uma_corrida(&mut afastados);
        passar_uma_corrida(&mut afastados_antigos);

        for r in &result.race_results {
            largadas += 1;
            contatos += r.incidents.iter().filter(|i| e_contato(i)).count() as u64;

            // A MESMA semente nas duas calibrações: a diferença medida é só o multiplicador.
            let semente = corrida.wrapping_mul(1013) ^ largadas.wrapping_mul(7919);

            // ── Calibração ATUAL. Uma lesão por piloto por corrida, e só se estiver são.
            if !afastados.contains_key(&r.pilot_id) {
                for inc in &r.incidents {
                    let mut rng = StdRng::seed_from_u64(semente);
                    if let Some(lesao) = generate_injury_from_incident(inc, 1, "R", &mut rng) {
                        match lesao.injury_type {
                            InjuryType::Leve => leves += 1,
                            InjuryType::Moderada => moderadas += 1,
                            _ => graves += 1,
                        }
                        afastados.insert(r.pilot_id.clone(), lesao.races_total);
                        break;
                    }
                }
            }

            // ── Calibração ANTIGA, nos mesmos incidentes.
            if !afastados_antigos.contains_key(&r.pilot_id) {
                for inc in &r.incidents {
                    let mut velho = inc.clone();
                    velho.injury_risk_multiplier = irm_antigo(inc);
                    let mut rng = StdRng::seed_from_u64(semente);
                    if let Some(lesao) = generate_injury_from_incident(&velho, 1, "R", &mut rng) {
                        lesoes_antigas += 1;
                        afastados_antigos.insert(r.pilot_id.clone(), lesao.races_total);
                        break;
                    }
                }
            }
        }

        // ── Peças: castigo dos contatos desta equipe, depois o desgaste normal da corrida.
        let mut pancadas: HashMap<String, u32> = HashMap::new();
        for r in &result.race_results {
            let n = r.incidents.iter().filter(|i| e_contato(i)).count() as u32;
            *pancadas.entry(r.team_id.clone()).or_insert(0) += n;
        }
        for (team_id, car) in carros.iter_mut() {
            let antes: f64 = car.parts.iter().map(|p| p.wear).sum();
            let hits = pancadas.get(team_id.as_str()).copied().unwrap_or(0);
            let dano = crate::car::crash::apply_contact_wear(car, CATEGORIA, hits);
            destruidas_por_contato += dano.destroyed.len() as u64;
            custo_de_contato += dano.cost;
            pior_rodada = pior_rodada.max(dano.cost);
            desgaste_de_contato += car.parts.iter().map(|p| p.wear).sum::<f64>() - antes;

            for p in car.parts.iter_mut() {
                p.wear += wear_per_race(p.part_type);
                if p.wear >= 1.0 {
                    // Equipe de meio de grid: troca quando a peça acaba, e paga por isso.
                    custo_de_manutencao += crate::car::cost::part_cost(CATEGORIA, p.part_type, p.level);
                    p.wear = 0.0;
                    p.spent = false;
                    trocas_por_fim_de_vida += 1;
                }
            }
        }
        historico_de_pancadas.push(pancadas);
    }

    let corridas = TEMPORADAS * ETAPAS;
    let equipes = grid.len() as f64;
    let total = leves + moderadas + graves;
    let pct = |n: u64| 100.0 * n as f64 / largadas as f64;
    let asa_por_corrida = wear_per_race(PartType::FrontWing);
    let contato_por_carro_corrida = desgaste_de_contato / largadas as f64;

    println!("\n═══ {corridas} corridas · {largadas} largadas · grid de {} ═══", grid.len());

    println!("\nLESÕES");
    println!(
        "  agora .................. {total:>5}   {:.2}% das largadas   {:.2}/corrida",
        pct(total),
        total as f64 / corridas as f64
    );
    println!(
        "  antes .................. {lesoes_antigas:>5}   {:.2}% das largadas",
        pct(lesoes_antigas)
    );
    println!(
        "  queda .................. {:.0}%",
        100.0 * (1.0 - total as f64 / lesoes_antigas.max(1) as f64)
    );
    println!("  mix .................... leve {leves} · moderada {moderadas} · GRAVE {graves}");
    println!("  save que motivou isto .. 11,70% das largadas");

    println!("\nCONTATOS");
    println!(
        "  total .................. {contatos}   {:.2}/corrida   {:.3}/carro/corrida",
        contatos as f64 / corridas as f64,
        contatos as f64 / largadas as f64
    );

    println!("\nPEÇAS — calibração atual ({CONTATO_ATUAL:.2} de wear por contato)");
    println!("  vida de uma asa ............. {:.0} corridas", 1.0 / asa_por_corrida);
    println!("  a asa gasta sozinha ......... {asa_por_corrida:.3}/corrida");
    println!(
        "  um contato custa ............ {:.0}% de uma corrida de vida de peça",
        100.0 * CONTATO_ATUAL / asa_por_corrida
    );
    println!("  desgaste extra .............. {contato_por_carro_corrida:.4}/carro/corrida");
    println!(
        "  destruídas por contato ...... {destruidas_por_contato}   {:.2}/equipe/temporada",
        destruidas_por_contato as f64 / (equipes * TEMPORADAS as f64)
    );
    println!(
        "  trocas por fim de vida ...... {trocas_por_fim_de_vida}   {:.1}/equipe/temporada (linha de base)",
        trocas_por_fim_de_vida as f64 / (equipes * TEMPORADAS as f64)
    );

    println!("\nORÇAMENTO — o canal que decidimos usar ({:.0}% do preço da peça por contato)",
        100.0 * crate::car::crash::CONTACT_COST_FRACTION);
    let temporadas_equipe = equipes * TEMPORADAS as f64;
    println!(
        "  manutenção normal ........... {:>12.0} por equipe/temporada",
        custo_de_manutencao / temporadas_equipe
    );
    println!(
        "  reparo de contato ........... {:>12.0} por equipe/temporada",
        custo_de_contato / temporadas_equipe
    );
    println!(
        "  peso do contato na conta .... {:>11.2}%",
        100.0 * custo_de_contato / custo_de_manutencao.max(1.0)
    );
    println!(
        "  pior rodada de uma equipe ... {:>12.0}  ({:.1}% de uma temporada de manutenção)",
        pior_rodada,
        100.0 * pior_rodada / (custo_de_manutencao / temporadas_equipe).max(1.0)
    );

    // ── Varredura: o mesmo fluxo de contatos, com castigos diferentes. É o que responde
    // "esse número é pouco ou muito?" olhando consequência em vez de intuição.
    println!("\nVARREDURA (mesmo fluxo de {contatos} contatos)");
    println!("  desgaste  destruídas/eq/temp   trocas a mais   |  fração  reparo/eq/temp   % da conta");
    for &w in &[0.06, 0.25, 0.50] {
        let (destruidas, trocas, _) = replay_pecas(&grid, &historico_de_pancadas, w, 0.10);
        let marca = if (w - CONTATO_ATUAL).abs() < 1e-9 { " ← atual" } else { "" };
        print!(
            "    {w:.2}         {:>6.2}            {:>+6.1}",
            destruidas as f64 / temporadas_equipe,
            (trocas as f64 - trocas_por_fim_de_vida as f64) / temporadas_equipe
        );
        println!("{marca}");
    }
    for &f in &[0.10, 0.25, 0.50, 1.00] {
        let (_, _, custo) = replay_pecas(&grid, &historico_de_pancadas, CONTATO_ATUAL, f);
        let marca = if (f - crate::car::crash::CONTACT_COST_FRACTION).abs() < 1e-9 {
            " ← atual"
        } else {
            ""
        };
        println!(
            "                                                |   {f:.2}   {:>10.0}     {:>6.2}%{marca}",
            custo / temporadas_equipe,
            100.0 * custo / custo_de_manutencao.max(1.0)
        );
    }
    println!();
}

/// Reexecuta SÓ o ciclo de vida das peças sobre um fluxo de contatos já gravado, variando o
/// castigo de desgaste e a fração de custo. Devolve `(destruídas, trocas, custo de reparo)`.
fn replay_pecas(
    grid: &[crate::simulation::context::SimDriver],
    historico: &[HashMap<String, u32>],
    wear_por_contato: f64,
    fracao_de_custo: f64,
) -> (u64, u64, f64) {
    let mut carros: HashMap<String, Car> = grid
        .iter()
        .map(|d| (d.team_id.clone(), Car::uniform(5)))
        .collect();
    let (mut destruidas, mut trocas, mut custo) = (0u64, 0u64, 0.0f64);

    for pancadas in historico {
        for (team_id, car) in carros.iter_mut() {
            let hits = pancadas.get(team_id.as_str()).copied().unwrap_or(0);
            let dano = crate::car::crash::apply_contact_wear_with(
                car,
                CATEGORIA,
                hits,
                wear_por_contato,
            );
            destruidas += dano.destroyed.len() as u64;
            // O custo sai proporcional: a função cobra com a constante de produção, e aqui a
            // reescalamos para a fração sob teste.
            custo += dano.cost / crate::car::crash::CONTACT_COST_FRACTION * fracao_de_custo;
            for p in car.parts.iter_mut() {
                p.wear += wear_per_race(p.part_type);
                if p.wear >= 1.0 {
                    p.wear = 0.0;
                    p.spent = false;
                    trocas += 1;
                }
            }
        }
    }
    (destruidas, trocas, custo)
}
