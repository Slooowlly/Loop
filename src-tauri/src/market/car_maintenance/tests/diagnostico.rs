//! DIAGNÓSTICO (`#[ignore]`): testes que não asseveram regra, e sim IMPRIMEM o
//! comportamento medido para quem for calibrar. Rode com `-- --ignored --nocapture`.

use super::super::*;
use super::*;
/// DIAGNÓSTICO — Pergunta 2 (rode com:
/// `cargo test analise_recorrencia_entre_corridas -- --ignored --nocapture`).
///
/// "Quando uma peça quebra, na PRÓXIMA corrida quebra de novo com a MESMA peça?"
///
/// FATO ARQUITETURAL que o teste torna visível: a quebra AO VIVO e a persistência do
/// desgaste são DESACOPLADAS. O pré-roll de quebra lê o desgaste de ENTRADA (persistido) e,
/// quando uma peça larga, zera o desgaste dela só NA SIMULAÇÃO (é descartado). O desgaste
/// que fica no save é avançado SÓ pelo cérebro de manutenção (`maintain_team_car` →
/// `advance_race`). Logo, "quebrar de novo" NÃO é causado pela quebra — é decidido pelo
/// ORÇAMENTO: time rico repõe a peça (desgaste zera → não repete); time pobre só degrada
/// (o desgaste passa da parede e a peça FORÇA falha toda corrida).
///
/// Roda o pipeline REAL por temporadas, para muitos times independentes de cada tier, e mede
/// a recorrência da MESMA peça em corridas consecutivas vs a taxa-base por peça.
#[test]
#[ignore]
fn analise_recorrencia_entre_corridas() {
    use crate::car::breakdown::{roll_race_breakdowns_cfg, Weather};
    use crate::car::PartType;
    use crate::models::team::placeholder_team_from_db;
    use std::collections::HashSet;

    const TIMES: usize = 600;
    const CORRIDAS: usize = 16;
    const CAT: &str = "gt3";
    let track_pha = (1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0); // pista equilibrada → mults ~1.0
    let weather = Weather::NEUTRAL;

    // Um tier = (nome, caixa, dívida, estado financeiro).
    // NB: o comportamento é um PENHASCO, não uma rampa. Caixas de 5e4 a 4e5 (times
    // placeholder SEM receita) deram todos idênticos ao POBRE — um carro GT3 completo custa
    // mais que isso pra repor, então só o topo escapa. Times reais têm receita recorrente e
    // caem entre RICO e POBRE; aqui o sinal honesto é o CONTRASTE rico↔degrada.
    let tiers: [(&str, f64, f64, &str); 3] = [
        ("RICO   (repõe tudo)", 1e12, 0.0, "healthy"),
        ("MÉDIO  (caixa 1.5e5)", 1.5e5, 0.0, "healthy"),
        ("POBRE  (só degrada)", 0.0, 1e9, "critical"),
    ];

    println!("\n================ RECORRÊNCIA DA MESMA PEÇA ENTRE CORRIDAS ================");
    println!(
        "  {} times × {} corridas cada, por tier. Pré-roll de quebra sobre o desgaste",
        TIMES, CORRIDAS
    );
    println!("  persistido; entre corridas o cérebro de manutenção avança/persiste o desgaste.\n");
    println!(
        "  {:<22} {:>10} {:>12} {:>9} {:>11} {:>10}",
        "tier", "base/peça", "recorrência", "razão", "DNF→carro", "forçada"
    );
    println!(
        "  {:<22} {:>10} {:>12} {:>9} {:>11} {:>10}",
        "", "P(quebra)", "P(mesma|N)", "rec/base", "some grid", "(parede)"
    );

    for (nome, cash, debt, estado) in tiers {
        let conn = Connection::open_in_memory().unwrap();

        // Contadores agregados.
        let mut breaks_partlevel = 0u64; // total de eventos (peça-nível) somados
        let mut race_slots = 0u64; // corridas × 11 peças (denominador da base)
        let mut prev_pairs = 0u64; // peças que quebraram numa corrida COM próxima corrida
        let mut recurred = 0u64; // ...dessas, quantas quebraram DE NOVO na seguinte
        let mut dnf_races = 0u64; // corridas em que o carro saiu (DNF)
        let mut total_races = 0u64;
        let mut forced_events = 0u64; // eventos por PAREDE (falha forçada, >HARD_WALL)
        let mut all_events = 0u64; // total de eventos (pra a fração forçada)

        for t in 0..TIMES {
            let team_id = format!("{}-{t}", nome.trim());
            let mut team = placeholder_team_from_db(
                team_id.clone(),
                team_id.clone(),
                CAT.to_string(),
                "2026-01-01T00:00:00".to_string(),
            );
            team.cash_balance = cash;
            team.debt_balance = debt;
            team.financial_state = estado.to_string();

            // Carro inicial: qualidade correlacionada ao tier (rico começa melhor), mas a
            // dinâmica de recorrência vem da manutenção corrida a corrida, não do seed.
            let q = if cash > 1e6 {
                0.7
            } else if cash > 0.0 {
                0.5
            } else {
                0.35
            };
            let car = seed_car(CAT, q);
            team_car::upsert_team_car(&conn, &team_id, &car).unwrap();
            team.car = Some(car);

            let mut prev: Option<HashSet<PartType>> = None;

            for r in 0..CORRIDAS {
                let car = team_car::get_team_car(&conn, &team_id).unwrap().unwrap();

                // Semente única por (time, corrida) — como o disparo ao vivo do jogo (1 sorte).
                let mut seed = 0xC0FF_EE00_u64 ^ (r as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
                for b in team_id.bytes() {
                    seed = seed
                        .wrapping_mul(0x0000_0100_0000_01B3)
                        .wrapping_add(b as u64);
                }
                let evs = roll_race_breakdowns_cfg(
                    &car,
                    18,
                    seed,
                    50.0,
                    track_pha,
                    weather,
                    &[],
                    false,
                    true,
                );

                let cur: HashSet<PartType> = evs.iter().map(|e| e.part).collect();
                total_races += 1;
                race_slots += 11;
                breaks_partlevel += cur.len() as u64;
                all_events += evs.len() as u64;
                forced_events += evs.iter().filter(|e| e.forced).count() as u64;
                if evs.iter().any(|e| e.is_dnf()) {
                    dnf_races += 1;
                }

                if let Some(prev_set) = &prev {
                    for &p in prev_set {
                        prev_pairs += 1;
                        if cur.contains(&p) {
                            recurred += 1;
                        }
                    }
                }
                prev = Some(cur);

                // FASE 5 — feedback físico: as peças que largaram viram consequência no save
                // (Leve segue; Grave→fim de vida; DNF→destruída/troca forçada). É o que corta
                // a recorrência (peça quebrada vira nova) e o runaway do pobre (vira dívida).
                let events: Vec<(PartType, crate::car::breakdown::Severity)> =
                    evs.iter().map(|e| (e.part, e.severity)).collect();
                // Entre corridas: o cérebro de manutenção avança/persiste o desgaste (neutro).
                maintain_team_car_pits(
                    &conn,
                    &team,
                    CAT,
                    1,
                    &[],
                    WearConditions::neutral(),
                    None,
                    false,
                    0,
                    &events,
                    0,
                )
                .unwrap();
                team.car = team_car::get_team_car(&conn, &team_id).unwrap();
            }
        }

        let base = breaks_partlevel as f64 / race_slots as f64;
        let recor = if prev_pairs > 0 {
            recurred as f64 / prev_pairs as f64
        } else {
            0.0
        };
        let razao = if base > 1e-9 { recor / base } else { 0.0 };
        let dnf = dnf_races as f64 / total_races as f64;
        let forced = if all_events > 0 {
            forced_events as f64 / all_events as f64
        } else {
            0.0
        };
        let recor_str = if prev_pairs > 0 {
            format!("{:>10.1}%", recor * 100.0)
        } else {
            "     —    ".to_string()
        };
        println!(
            "  {:<22} {:>9.1}% {} {:>7.1}× {:>10.1}% {:>9.1}%",
            nome,
            base * 100.0,
            recor_str,
            razao,
            dnf * 100.0,
            forced * 100.0,
        );
    }
    println!("\n  Leitura: 'base/peça' = chance de UMA peça qualquer quebrar numa corrida.");
    println!("  'recorrência' = dado que a peça quebrou, chance de a MESMA quebrar na próxima.");
    println!("  'razão' ≫ 1 = a quebra é PEGAJOSA (a mesma peça repete muito acima do acaso).\n");
}

// -------- As 11 peças desgastam de forma diferente? (staggering) --------

/// DIAGNÓSTICO (rode com:
/// `cargo test analise_desgaste_por_peca -- --ignored --nocapture`).
///
/// "As 11 peças deveriam desgastar de forma diferente." Este teste TORNA VISÍVEL o
/// desgaste PERSISTIDO peça a peça, corrida a corrida, num carro rico no teto (que só cicla
/// por fim-de-vida). Imprime o wear de cada peça e marca `*` quando entra na zona de risco
/// (≥ 87%, quebraria na próxima). Compara calendário NEUTRO vs VARIADO.
///
/// O que ele expõe: no persistido NÃO há ruído (só `wear_per_race × pista × clima`); todas
/// largam em wear 0 iguais. Logo peças de MESMA durabilidade (há 6 de durab 3!) só se
/// separam pela pista/clima. Num calendário neutro elas marcham em LOCKSTEP e chegam ao
/// fim-de-vida JUNTAS — a origem do "várias peças quebram na mesma corrida".
#[test]
#[ignore]
fn analise_desgaste_por_peca() {
    // Abreviações de 3 letras, na ordem de PartType::ALL.
    let abbr = |pt: PartType| match pt {
        PartType::Chassis => "Cha",
        PartType::Engine => "Eng",
        PartType::FrontWing => "AsD",
        PartType::RearWing => "AsT",
        PartType::Underbody => "Ass",
        PartType::Sidepods => "Sid",
        PartType::Cooling => "Arr",
        PartType::Gearbox => "Cbx",
        PartType::Brakes => "Fre",
        PartType::Suspension => "Sus",
        PartType::Electronics => "Ele",
    };
    // A abertura da janela de risco, direto da FONTE. Era uma cópia manual aqui, e ela já ficou
    // parada em 0.87 depois que a janela abriu em 0.90 — o mesmo modo de falha do harness de
    // Monte Carlo, num arquivo que nenhuma asserção protege (isto só marca o `*` do relatório).
    const RISK_OPEN: f64 = crate::car::breakdown::WEAR_RISK_OPEN;

    // Calendário neutro (tudo equilibrado) vs variado (potência→handling→aceleração).
    let neutro = [(1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0)];
    let variado = [(0.70, 0.15, 0.15), (0.15, 0.70, 0.15), (0.20, 0.15, 0.65)];
    let quente = crate::car::breakdown::Weather {
        wetness: 0.0,
        temperature: 32.0,
        humidity: 80.0,
        wind_kmh: 30.0,
    };

    for (nome, calendario, usar_clima) in [
        (
            "NEUTRO  (pista equilibrada, clima neutro)",
            &neutro[..],
            false,
        ),
        ("VARIADO (P→H→A rotando, +1 dia quente)", &variado[..], true),
    ] {
        println!("\n================ DESGASTE PERSISTIDO POR PEÇA — {nome} ================");
        // Cabeçalho: durabilidade de cada peça (o diferenciador principal).
        print!("  {:>7}", "durab:");
        for &pt in &PartType::ALL {
            print!(" {:>3}", pt.durability());
        }
        println!();
        print!("  {:>7}", "corrida");
        for &pt in &PartType::ALL {
            print!(" {:>3}", abbr(pt));
        }
        println!("     (peças na zona de risco ≥90%)");

        let mut car = Car::uniform(7); // GT3 no teto → só cicla por fim-de-vida
        for r in 0..14 {
            let track = calendario[r % calendario.len()];
            let demand = track;
            // Clima quente numa corrida a cada 3 (só no cenário variado).
            let weather = if usar_clima && r % 3 == 2 {
                quente
            } else {
                crate::car::breakdown::Weather::NEUTRAL
            };

            // Estado PERSISTIDO no INÍCIO desta corrida = o que o pré-roll de quebra leria.
            // "Em risco" = a peça CRUZA a zona (≥90%) DURANTE esta corrida (entrada +
            // desgaste da corrida), e também a que já entrou acima de 90%.
            let cruza_zona = |pt: PartType, w: f64| w + wear_per_race(pt) >= RISK_OPEN;
            let em_risco: Vec<&str> = PartType::ALL
                .iter()
                .filter(|&&pt| {
                    car.part(pt)
                        .map(|p| cruza_zona(pt, p.wear))
                        .unwrap_or(false)
                })
                .map(|&pt| abbr(pt))
                .collect();
            print!("  {:>7}", format!("→{}", r + 1));
            for &pt in &PartType::ALL {
                let w = car.part(pt).map(|p| p.wear).unwrap_or(0.0);
                let mark = if cruza_zona(pt, w) { "*" } else { " " };
                print!(" {:>2.0}{}", w * 100.0, mark);
            }
            if em_risco.is_empty() {
                println!("   —");
            } else {
                println!("   {} ({})", em_risco.join(","), em_risco.len());
            }

            // Avança a corrida (cérebro rico repõe no fim-de-vida; clima/pista modulam).
            let plan = decide_maintenance(&car, "gt3", 1e12, demand);
            let wear_mults = crate::car::breakdown::conditions_wear_mults(track, weather);
            apply_plan_scaled(&mut car, &plan, &wear_mults, true, 1.0);
        }
    }
    println!(
        "\n  Leitura: peças de MESMA durabilidade e MESMO perfil (ex.: AsD/AsT, ambas durab 3)"
    );
    println!("  entram na zona (*) JUNTAS no neutro. A pista/clima é o ÚNICO desempate — sem ela,");
    println!("  o desgaste persistido não diferencia peças de mesma durabilidade.\n");
}
