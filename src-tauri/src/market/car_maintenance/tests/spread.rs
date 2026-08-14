//! O SPREAD da grade: o mesmo carro em todo mundo, e só o orçamento separando — do seed
//! inicial ao teto da categoria, passando pela temporada inteira pelo banco.

use super::super::*;
use super::*;
#[test]
fn seed_persiste_carros_correlacionados_com_a_categoria() {
    use crate::models::team::placeholder_team_from_db;
    let conn = Connection::open_in_memory().unwrap();
    let mk = |id: &str, cat: &str, cp: f64| {
        let mut t = placeholder_team_from_db(
            id.to_string(),
            format!("Team {id}"),
            cat.to_string(),
            "2026-01-01T00:00:00".to_string(),
        );
        t.car_performance = cp;
        t
    };
    let teams = vec![
        mk("A", "gt3", 15.0),         // topo do GT3
        mk("B", "gt3", 5.0),          // fundo do GT3
        mk("R", "mazda_rookie", 2.0), // spec
    ];

    seed_and_persist_team_cars(&conn, &teams).unwrap();

    let a = team_car::get_team_car(&conn, "A").unwrap().unwrap();
    let b = team_car::get_team_car(&conn, "B").unwrap().unwrap();
    let r = team_car::get_team_car(&conn, "R").unwrap().unwrap();
    assert!(
        a.display_level() > b.display_level(),
        "topo do GT3 ({}) deveria ter carro melhor que o fundo ({})",
        a.display_level(),
        b.display_level()
    );
    assert!(a.display_level() <= 7, "respeita o teto do GT3");
    assert_eq!(r.display_level(), 1, "rookie é spec");
}

/// Integração de ponta a ponta PELO BANCO: seed → carrega → cérebro decide →
/// desgaste → persiste, por uma temporada. Todos começam iguais (nível 5); só o
/// ORÇAMENTO os separa. Demonstra o spread da grade emergindo do dinheiro — a
/// promessa central do sistema. Exercita seed + wear + brain + team_car juntos.
#[test]
fn integracao_temporada_o_orcamento_abre_o_spread_da_grade() {
    let conn = Connection::open_in_memory().unwrap();
    let rich = ["R1", "R2", "R3"];
    let poor = ["P1", "P2", "P3"];

    // Todos nascem no mesmo carro (gt3, qualidade média → nível 5).
    for id in rich.iter().chain(poor.iter()) {
        team_car::upsert_team_car(&conn, id, &seed_car("gt3", 0.5)).unwrap();
    }
    // Sanidade: largaram iguais.
    for id in rich.iter().chain(poor.iter()) {
        assert_eq!(
            team_car::get_team_car(&conn, id)
                .unwrap()
                .unwrap()
                .display_level(),
            5
        );
    }

    // Calendário misto de uma temporada: power → handling → accel, repetindo.
    let calendario = [(0.70, 0.20, 0.10), (0.15, 0.70, 0.15), (0.20, 0.15, 0.65)];

    for corrida in 0..24 {
        let demand = calendario[corrida % calendario.len()];
        for (&id, budget) in rich.iter().zip(std::iter::repeat(1e12)) {
            let mut car = team_car::get_team_car(&conn, id).unwrap().unwrap();
            let plan = decide_maintenance(&car, "gt3", budget, demand);
            apply_plan(&mut car, &plan);
            team_car::upsert_team_car(&conn, id, &car).unwrap();
        }
        for (&id, budget) in poor.iter().zip(std::iter::repeat(0.0)) {
            let mut car = team_car::get_team_car(&conn, id).unwrap().unwrap();
            let plan = decide_maintenance(&car, "gt3", budget, demand);
            apply_plan(&mut car, &plan);
            team_car::upsert_team_car(&conn, id, &car).unwrap();
        }
    }

    let nivel = |id: &str| {
        team_car::get_team_car(&conn, id)
            .unwrap()
            .unwrap()
            .display_level()
    };
    let ricos: Vec<u8> = rich.iter().map(|id| nivel(id)).collect();
    let pobres: Vec<u8> = poor.iter().map(|id| nivel(id)).collect();

    // Os ricos sobem rumo ao teto (7); os pobres sangram; todos respeitam o teto.
    assert!(
        ricos.iter().all(|&l| l >= 6),
        "ricos deveriam chegar perto do teto: {ricos:?}"
    );
    assert!(
        pobres.iter().all(|&l| l <= 3),
        "pobres deveriam sangrar: {pobres:?}"
    );
    assert!(
        ricos.iter().all(|&l| l <= 7),
        "ninguém passa do teto do GT3: {ricos:?}"
    );
    let melhor_pobre = *pobres.iter().max().unwrap();
    let pior_rico = *ricos.iter().min().unwrap();
    assert!(
        pior_rico > melhor_pobre + 2,
        "o spread deveria ser nítido: ricos={ricos:?} pobres={pobres:?}"
    );
}

#[test]
fn tick_por_rodada_evolui_e_persiste_o_carro_pelo_caixa() {
    use crate::models::team::placeholder_team_from_db;
    let conn = Connection::open_in_memory().unwrap();
    let mut team = placeholder_team_from_db(
        "T1".to_string(),
        "Team 1".to_string(),
        "gt3".to_string(),
        "2026-01-01T00:00:00".to_string(),
    );
    team.cash_balance = 1e12;
    team.debt_balance = 0.0;
    team.financial_state = "healthy".to_string();

    // Carro inicial no piso do GT3 (nível 3), persistido e anexado.
    let car = seed_car("gt3", 0.0);
    let start = car.display_level();
    team_car::upsert_team_car(&conn, "T1", &car).unwrap();
    team.car = Some(car);

    // Várias rodadas com caixa alto → o carro sobe rumo ao teto, e cada rodada tem custo.
    for _ in 0..20 {
        let cost = maintain_team_car(
            &conn,
            &team,
            "gt3",
            1,
            &[239, 489, 324],
            WearConditions::neutral(),
            None,
        )
        .unwrap();
        assert!(cost >= 0.0);
        team.car = team_car::get_team_car(&conn, "T1").unwrap();
    }

    let end = team.car.as_ref().unwrap().display_level();
    assert!(
        end > start,
        "carro de time rico deveria melhorar (start={start}, end={end})"
    );
}

#[test]
fn carro_acima_do_teto_regride_ao_entrar_na_categoria() {
    use crate::models::team::placeholder_team_from_db;
    let conn = Connection::open_in_memory().unwrap();
    let mut team = placeholder_team_from_db(
        "T1".to_string(),
        "Rebaixado".to_string(),
        "mazda_amador".to_string(),
        "2026-01-01T00:00:00".to_string(),
    );
    // Carro alto (nível 6), como se o time tivesse caído de uma categoria superior.
    let high = Car::uniform(6);
    team_car::upsert_team_car(&conn, "T1", &high).unwrap();
    team.car = Some(high);

    // Amador tem teto natural 2 e teto de DESENVOLVIMENTO 4 (a parede: dois níveis acima, ao
    // preço que o design §6 chama de inviável). O carro de nível 6 tem de cair ao que a
    // categoria admite — não ao teto natural, porque acima dele existe território legítimo de
    // quem sangra dinheiro, e sim ao topo da parede. O que o teste guarda é que o carro
    // herdado NÃO atravessa a categoria intacto.
    let teto_da_parede = crate::car::cost::development_ceiling(team.car_ceiling());
    maintain_team_car(
        &conn,
        &team,
        "mazda_amador",
        1,
        &[],
        WearConditions::neutral(),
        None,
    )
    .unwrap();

    let after = team_car::get_team_car(&conn, "T1").unwrap().unwrap();
    assert_eq!(teto_da_parede, 4, "amador: teto natural 2 + dois da parede");
    assert!(
        after.display_level() <= teto_da_parede,
        "carro deveria regredir ao teto da parede do amador ({teto_da_parede}), ficou {}",
        after.display_level()
    );
    assert!(
        after.display_level() < 6,
        "o carro herdado não pode atravessar a categoria intacto, ficou {}",
        after.display_level()
    );
}

#[test]
fn calendario_peaked_faz_o_carro_especializar() {
    // Demanda (P, H, A) fortemente de POWER → o carro foca em power.
    let power_demand = (0.70, 0.15, 0.15);
    let mut car = Car::uniform(1);
    for _ in 0..30 {
        let plan = decide_maintenance(&car, "gt3", 1e12, power_demand);
        apply_plan(&mut car, &plan);
    }
    let engine = car.part(PartType::Engine).unwrap().level; // relevante (power)
    let brakes = car.part(PartType::Brakes).unwrap().level; // irrelevante (H puro)
    assert!(
        engine > brakes + 1,
        "carro deveria focar em power (motor {engine} vs freios {brakes})"
    );
    let (p, h, _a) = car.pha();
    assert!(p > h, "o shape deveria pesar power: P={p:.1} H={h:.1}");
}

// -------- DNA / identidade de carro do time --------

#[test]
fn calendario_equilibrado_mantem_carro_parelho() {
    let balanced = (0.34, 0.33, 0.33);
    let mut car = Car::uniform(1);
    for _ in 0..30 {
        let plan = decide_maintenance(&car, "gt3", 1e12, balanced);
        apply_plan(&mut car, &plan);
    }
    let levels: Vec<u8> = car.parts.iter().map(|p| p.level).collect();
    let max = *levels.iter().max().unwrap();
    let min = *levels.iter().min().unwrap();
    assert!(
        max - min <= 1,
        "equilibrado deveria manter o carro parelho: {levels:?}"
    );
}

#[test]
fn time_rico_atinge_o_teto_e_pobre_sangra() {
    // Calendário equilibrado → sem especialização; testa a magnitude/nível puro.
    let demand = (0.34, 0.33, 0.33);

    let mut rich = Car::uniform(3);
    for _ in 0..25 {
        let plan = decide_maintenance(&rich, "gt3", 1e12, demand);
        apply_plan(&mut rich, &plan);
    }
    assert!(
        rich.display_level() >= 6,
        "time rico deveria chegar perto do teto 7, ficou {}",
        rich.display_level()
    );

    let mut poor = Car::uniform(3);
    for _ in 0..25 {
        let plan = decide_maintenance(&poor, "gt3", 0.0, demand);
        apply_plan(&mut poor, &plan);
    }
    assert!(
        poor.display_level() < 3,
        "time pobre deveria sangrar abaixo de 3, ficou {}",
        poor.display_level()
    );
}

// -------- Recorrência da quebra ENTRE corridas (Pergunta 2) --------
