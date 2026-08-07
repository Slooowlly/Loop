//! Suíte de testes do cérebro de manutenção do carro (extraída de
//! `car_maintenance.rs`).
//!
//! Continua sendo o mesmo módulo `tests` de antes: `use super::*` enxerga o
//! módulo `car_maintenance` inteiro, incluindo os itens privados.

use super::*;

// -------- Horizonte de planejamento --------

#[test]
fn horizonte_distribuicao_bate_20_30_30_20() {
    let n = 3000;
    let (mut single, mut three, mut five, mut season) = (0, 0, 0, 0);
    for i in 0..n {
        match planning_horizon(&format!("T{i}"), 1) {
            PlanningHorizon::SingleTrack => single += 1,
            PlanningHorizon::ThreeRaces => three += 1,
            PlanningHorizon::FiveRaces => five += 1,
            PlanningHorizon::FullSeason => season += 1,
        }
    }
    let frac = |x: i32| x as f64 / n as f64;
    assert!(
        (frac(single) - 0.20).abs() < 0.04,
        "single={}",
        frac(single)
    );
    assert!((frac(three) - 0.30).abs() < 0.04, "three={}", frac(three));
    assert!((frac(five) - 0.30).abs() < 0.04, "five={}", frac(five));
    assert!(
        (frac(season) - 0.20).abs() < 0.04,
        "season={}",
        frac(season)
    );
}

#[test]
fn horizonte_e_deterministico() {
    assert_eq!(
        planning_horizon("Team-42", 7),
        planning_horizon("Team-42", 7)
    );
}

#[test]
fn horizonte_re_rola_por_temporada() {
    let n = 500;
    let changed = (0..n)
        .filter(|i| planning_horizon(&format!("T{i}"), 1) != planning_horizon(&format!("T{i}"), 2))
        .count();
    assert!(
        changed > 200,
        "esperado muitos times re-rolando; mudaram {changed}"
    );
}

// -------- Demanda de manutenção --------

#[test]
fn demanda_normaliza_e_le_as_pistas() {
    // Vazio → balanceado.
    let (p, h, a) = maintenance_demand(&[]);
    assert!((p + h + a - 1.0).abs() < 1e-9);
    assert!((p - 1.0 / 3.0).abs() < 1e-9);
    // Monza (239) é power-heavy → P domina.
    let (p2, h2, a2) = maintenance_demand(&[239]);
    assert!((p2 + h2 + a2 - 1.0).abs() < 1e-6);
    assert!(
        p2 > h2 && p2 > a2,
        "Monza deveria exigir Power: P={p2} H={h2} A={a2}"
    );
}

// -------- Decisão de manutenção --------

#[test]
fn prioriza_peca_do_atributo_exigido_com_caixa_curto() {
    let mut car = Car::uniform(5);
    car.set_wear(PartType::Engine, 0.90); // fim de vida
    car.set_wear(PartType::Brakes, 0.90); // fim de vida
    let demand = (1.0, 0.0, 0.0); // power puro
    let budget = replace_cost("gt4", car.part(PartType::Engine).unwrap());

    let plan = decide_maintenance(&car, "gt4", budget, demand);

    // O motor (relevante em power) leva a única troca possível...
    assert_eq!(
        plan.actions.get(&PartType::Engine),
        Some(&PartAction::Replace)
    );
    // ...e os freios (H puro, irrelevantes aqui, sem caixa) degradam.
    assert_eq!(
        plan.actions.get(&PartType::Brakes),
        Some(&PartAction::Degrade)
    );
}

#[test]
fn estica_quando_sem_caixa_mas_a_pista_exige() {
    let mut car = Car::uniform(5);
    car.set_wear(PartType::Engine, 0.90);
    let demand = (1.0, 0.0, 0.0);
    // Caixa só dá para esticar (40% de uma nova), não para trocar.
    let sc = stretch_cost("gt4", car.part(PartType::Engine).unwrap());

    let plan = decide_maintenance(&car, "gt4", sc, demand);

    assert_eq!(
        plan.actions.get(&PartType::Engine),
        Some(&PartAction::Stretch)
    );
}

#[test]
fn degrada_peca_irrelevante_para_a_proxima_pista() {
    let mut car = Car::uniform(5);
    car.set_wear(PartType::Brakes, 0.90);
    let demand = (1.0, 0.0, 0.0); // power; freios (H puro) irrelevantes

    let plan = decide_maintenance(&car, "gt4", 0.0, demand);

    assert_eq!(
        plan.actions.get(&PartType::Brakes),
        Some(&PartAction::Degrade)
    );
}

// -------- Cadência de desenvolvimento --------

/// A cota da janela é o freio: fora dela o time não sobe nível nenhum, por mais caixa
/// que tenha. É o que impede o recém-promovido de igualar o campo num fim de semana.
#[test]
fn fora_da_janela_o_time_nao_sobe_nivel_nem_com_caixa_infinito() {
    let car = Car::uniform(1);
    let demand = (0.34, 0.33, 0.33);

    let plan = decide_maintenance_with_limits(&car, "gt4", 1e12, demand, None, Some(0));

    assert!(
        plan.target_levels.is_empty(),
        "sem cota, nenhuma peça pode subir"
    );
}

/// Dentro da janela sobe UMA peça — não as onze. A escolhida é a mais relevante para a
/// demanda, que é o que faz o foco importar quando os upgrades são poucos.
#[test]
fn na_janela_sobe_apenas_a_cota_e_pela_relevancia() {
    let car = Car::uniform(1);
    let power = (1.0, 0.0, 0.0);

    let plan = decide_maintenance_with_limits(&car, "gt4", 1e12, power, None, Some(1));

    assert_eq!(plan.target_levels.len(), 1, "a cota é de uma peça");
    assert_eq!(
        plan.target_levels.get(&PartType::Engine),
        Some(&2),
        "com demanda de power puro, o motor leva o upgrade"
    );
}

/// Sem limite (chamada pura / harness) o comportamento antigo continua valendo — é o que
/// mantém os testes de shape e o Monte Carlo medindo o carro completo.
#[test]
fn sem_limite_o_passe_de_upgrade_sobe_o_carro_todo() {
    let car = Car::uniform(1);
    let demand = (0.34, 0.33, 0.33);

    let plan = decide_maintenance_with_limits(&car, "gt4", 1e12, demand, None, None);

    assert_eq!(plan.target_levels.len(), PartType::ALL.len());
}

/// A cadência dá 3–4 janelas numa temporada de 12–16 etapas, e times diferentes não
/// desenvolvem todos na mesma rodada.
#[test]
fn a_cadencia_da_tres_a_quatro_janelas_por_temporada() {
    for etapas in 12..=16 {
        for team_id in ["T001", "T042", "MA3"] {
            let janelas: u32 = (0..etapas)
                .map(|rodada| upgrades_permitidos_nesta_corrida(team_id, etapas - 1 - rodada))
                .sum();
            assert!(
                (3..=4).contains(&janelas),
                "{team_id} em {etapas} etapas teve {janelas} janelas"
            );
        }
    }
}

// -------- Seed inicial --------

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
fn dna_e_estavel_e_nao_depende_de_temporada() {
    // Determinístico e permanente (a assinatura nem recebe temporada).
    assert_eq!(team_car_focus("Team-42"), team_car_focus("Team-42"));
}

#[test]
fn dna_distribuicao_40_20_20_20() {
    let n = 3000;
    let (mut b, mut p, mut h, mut a) = (0, 0, 0, 0);
    for i in 0..n {
        match team_car_focus(&format!("T{i}")) {
            CarFocus::Balanced => b += 1,
            CarFocus::Power => p += 1,
            CarFocus::Handling => h += 1,
            CarFocus::Acceleration => a += 1,
        }
    }
    let frac = |x: i32| x as f64 / n as f64;
    assert!((frac(b) - 0.40).abs() < 0.05, "balanced={}", frac(b));
    assert!((frac(p) - 0.20).abs() < 0.05, "power={}", frac(p));
    assert!((frac(h) - 0.20).abs() < 0.05, "handling={}", frac(h));
    assert!((frac(a) - 0.20).abs() < 0.05, "accel={}", frac(a));
}

#[test]
fn dna_pica_a_demanda_que_o_calendario_diverso_lavaria() {
    // Calendário diverso (2 P + 1 H + 1 A) → média ~balanceada, spread abaixo do gatilho.
    let diverse = maintenance_demand(&[239, 523, 489, 179]); // Monza+Spa(P) Ledenon(H) LongBeach(A)
    assert!(
        demand_spread(diverse) < DEMAND_PEAK_THRESHOLD,
        "calendário diverso deveria lavar pra balanceado: spread={}",
        demand_spread(diverse)
    );
    // DNA de potência empurra a demanda efetiva acima do gatilho.
    let blended = blend_with_focus(diverse, CarFocus::Power);
    let (p, h, a) = blended;
    assert!(
        p > h && p > a,
        "DNA deveria puxar power: P={p:.2} H={h:.2} A={a:.2}"
    );
    assert!(
        demand_spread(blended) >= DEMAND_PEAK_THRESHOLD,
        "DNA deveria peakar a demanda: spread={}",
        demand_spread(blended)
    );
    // DNA balanceado NÃO peaka (generalista continua generalista).
    assert!(demand_spread(blend_with_focus(diverse, CarFocus::Balanced)) < DEMAND_PEAK_THRESHOLD);
}

#[test]
fn time_com_dna_de_potencia_foca_mesmo_em_calendario_diverso() {
    use crate::models::team::placeholder_team_from_db;
    // Acha um id cujo DNA é potência.
    let team_id = (0..10_000)
        .map(|i| format!("P{i}"))
        .find(|id| team_car_focus(id) == CarFocus::Power)
        .expect("deveria existir id com DNA de potência");

    let conn = Connection::open_in_memory().unwrap();
    let mut team = placeholder_team_from_db(
        team_id.clone(),
        "Power DNA".to_string(),
        "gt3".to_string(),
        "2026-01-01T00:00:00".to_string(),
    );
    team.cash_balance = 1e12;
    team.debt_balance = 0.0;
    team.financial_state = "healthy".to_string();
    let car = seed_car("gt3", 0.5);
    team_car::upsert_team_car(&conn, &team_id, &car).unwrap();
    team.car = Some(car);

    // Calendário DIVERSO cuja média até pende de leve pra HANDLING (contra o DNA):
    // se mesmo assim o carro pesa power, é o DNA sustentando o foco, não o calendário.
    let diverse = [489, 325, 180, 318, 93, 188];
    for season in 1..=4 {
        for _ in 0..15 {
            maintain_team_car(
                &conn,
                &team,
                "gt3",
                season,
                &diverse,
                WearConditions::neutral(),
                None,
            )
            .unwrap();
            team.car = team_car::get_team_car(&conn, &team_id).unwrap();
        }
    }

    let car = team.car.as_ref().unwrap();
    let (p, h, a) = car.pha();
    assert!(
        p > h && p > a,
        "carro de DNA-power deveria pesar power num calendário diverso: P={p:.1} H={h:.1} A={a:.1}"
    );
    let engine = car.part(PartType::Engine).unwrap().level;
    let brakes = car.part(PartType::Brakes).unwrap().level;
    assert!(
        engine > brakes,
        "peça de power (motor {engine}) deveria superar a de handling (freios {brakes})"
    );
}

// -------- Feedback físico da quebra (§4.6) --------

/// Roda a manutenção de UM carro com um desgaste inicial no motor e uma lista de quebras
/// desta corrida; devolve `(custo, peça-motor persistida)`.
fn maintain_com_quebra(
    cash: f64,
    debt: f64,
    state: &str,
    engine_wear: f64,
    events: &[(PartType, crate::car::breakdown::Severity)],
) -> (f64, CarPart) {
    // Time de DNA BALANCEADO (sem foco) → o cérebro não de-investe nenhuma peça: uma peça no
    // fim de vida é trocada (com caixa) em vez de degradada. Isola o efeito do feedback.
    let team_id = (0..2000)
        .map(|i| format!("T{i}"))
        .find(|id| team_car_focus(id) == CarFocus::Balanced)
        .unwrap();
    maintain_com_quebra_team(&team_id, cash, debt, state, engine_wear, events)
}

fn maintain_com_quebra_team(
    team_id: &str,
    cash: f64,
    debt: f64,
    state: &str,
    engine_wear: f64,
    events: &[(PartType, crate::car::breakdown::Severity)],
) -> (f64, CarPart) {
    use crate::models::team::placeholder_team_from_db;
    let conn = Connection::open_in_memory().unwrap();
    let mut team = placeholder_team_from_db(
        team_id.to_string(),
        team_id.to_string(),
        "gt3".to_string(),
        "2026-01-01T00:00:00".to_string(),
    );
    team.cash_balance = cash;
    team.debt_balance = debt;
    team.financial_state = state.to_string();
    let mut car = Car::uniform(5);
    car.set_wear(PartType::Engine, engine_wear);
    team_car::upsert_team_car(&conn, team_id, &car).unwrap();
    team.car = Some(car);
    let cost = maintain_team_car_pits(
        &conn,
        &team,
        "gt3",
        1,
        &[],
        WearConditions::neutral(),
        None,
        false,
        0,
        events,
        0,
    )
    .unwrap();
    let after = team_car::get_team_car(&conn, team_id).unwrap().unwrap();
    let engine = *after.part(PartType::Engine).unwrap();
    (cost, engine)
}

/// Roda a manutenção de um time pobre com `hits` contatos de disputa e a asa dianteira
/// entrando na corrida com `front_wing_wear`. Devolve `(custo, asa depois)`.
fn maintain_com_contatos(front_wing_wear: f64, hits: u32) -> (f64, CarPart) {
    use crate::models::team::placeholder_team_from_db;
    // DNA balanceado: o cérebro troca a peça no fim de vida em vez de degradá-la — isola o
    // efeito do contato (mesma razão de `maintain_com_quebra`).
    let team_id = (0..2000)
        .map(|i| format!("T{i}"))
        .find(|id| team_car_focus(id) == CarFocus::Balanced)
        .unwrap();
    let conn = Connection::open_in_memory().unwrap();
    let mut team = placeholder_team_from_db(
        team_id.clone(),
        team_id.clone(),
        "gt3".to_string(),
        "2026-01-01T00:00:00".to_string(),
    );
    // Time POBRE: sem caixa e já endividado. O que ele gastar aqui é dívida.
    team.cash_balance = 0.0;
    team.debt_balance = 1e9;
    team.financial_state = "critical".to_string();
    let mut car = Car::uniform(5);
    car.set_wear(PartType::FrontWing, front_wing_wear);
    team_car::upsert_team_car(&conn, &team_id, &car).unwrap();
    team.car = Some(car);
    let cost = maintain_team_car_pits(
        &conn,
        &team,
        "gt3",
        1,
        &[],
        WearConditions::neutral(),
        None,
        false,
        0,
        &[],
        hits,
    )
    .unwrap();
    let after = team_car::get_team_car(&conn, &team_id).unwrap().unwrap();
    let asa = *after.part(PartType::FrontWing).unwrap();
    (cost, asa)
}

/// **Bater tem preço.** Até aqui um roda-a-roda da IA não deixava marca nenhuma no carro: o
/// contato custava tempo dentro da corrida e evaporava. Agora ele castiga a peça, e uma corrida
/// cheia de contato chega na seguinte com a asa mais perto do fim que uma corrida limpa.
#[test]
fn contato_de_disputa_desgasta_mais_que_corrida_limpa() {
    let (_, asa_limpa) = maintain_com_contatos(0.0, 0);
    let (_, asa_batida) = maintain_com_contatos(0.0, 6);

    assert!(
        asa_batida.wear > asa_limpa.wear,
        "asa de quem bateu 6× deveria estar mais gasta: batida={} limpa={}",
        asa_batida.wear,
        asa_limpa.wear
    );
}

/// **E bater com a peça no fim manda o time pro vermelho.** A asa que já estava acabada quando
/// levou o contato é destruída → troca FORÇADA, mesmo sem caixa. É o elo que faltava entre a
/// batida da IA e o orçamento dela: o time pobre que corre no soco vira dívida, e a peça volta
/// NOVA (não fica presa em sobreuso, requebrando pra sempre).
#[test]
fn contato_em_peca_acabada_forca_troca_a_debito() {
    // 0.90 está acima do limiar de destruição (0.85) de `car::crash`.
    let (cost, asa) = maintain_com_contatos(0.90, 1);

    assert!(
        cost > 0.0,
        "a troca forçada tem de cobrar, mesmo sem caixa (vira dívida); custo={cost}"
    );
    assert!(
        asa.wear < 0.90,
        "a asa destruída tem de voltar NOVA, não seguir acabada; wear={}",
        asa.wear
    );
    assert!(
        !asa.spent,
        "peça reposta não pode nascer marcada como esgotada"
    );
}

#[test]
fn dnf_destroi_e_repoe_a_peca_mesmo_sem_caixa() {
    use crate::car::breakdown::Severity;
    // Time POBRE (sem caixa), motor a MEIA-VIDA (não estava no fim). DNF destrói → troca
    // FORÇADA a débito: a peça vira NOVA (não fica presa em sobreuso) e há custo cobrado.
    let (cost, engine) = maintain_com_quebra(
        0.0,
        1e9,
        "critical",
        0.30,
        &[(PartType::Engine, Severity::Dnf)],
    );
    assert!(
        engine.wear < 0.5,
        "motor destruído deveria virar NOVO (wear baixo), deu {}",
        engine.wear
    );
    assert!(
        cost > 0.0,
        "a troca forçada do DNF deveria cobrar custo (a débito), deu {cost}"
    );
}

#[test]
fn sem_feedback_a_peca_do_pobre_so_acumula() {
    // Contraste: MESMO cenário, sem evento → o motor a 0.30 num time pobre só acumula, NÃO
    // vira novo. Prova que é o FEEDBACK (não o cérebro) que reseta a peça no DNF.
    let (_c, engine) = maintain_com_quebra(0.0, 1e9, "critical", 0.30, &[]);
    assert!(
        engine.wear > 0.4,
        "sem quebra, o motor só acumula (não reseta): {}",
        engine.wear
    );
}

#[test]
fn leve_nao_altera_a_peca() {
    use crate::car::breakdown::Severity;
    // Leve = mesmo desfecho que SEM quebra (a peça só perdeu rendimento na corrida).
    let (_c1, com) = maintain_com_quebra(
        0.0,
        1e9,
        "critical",
        0.30,
        &[(PartType::Engine, Severity::Light)],
    );
    let (_c2, sem) = maintain_com_quebra(0.0, 1e9, "critical", 0.30, &[]);
    assert!(
        (com.wear - sem.wear).abs() < 1e-9,
        "Leve não deveria mudar a peça (com {} vs sem {})",
        com.wear,
        sem.wear
    );
}

#[test]
fn grave_forca_troca_ate_sem_caixa() {
    use crate::car::breakdown::Severity;
    // GRAVE também força a troca (variante simples) — inclusive no time POBRE, a débito: a
    // peça que custou tempo vira NOVA e não requebra; o buraco é financeiro (custo cobrado).
    let (cost, engine) = maintain_com_quebra(
        0.0,
        1e9,
        "critical",
        0.30,
        &[(PartType::Engine, Severity::Heavy)],
    );
    assert!(
        engine.wear < 0.5,
        "Grave deveria trocar a peça (nova) mesmo sem caixa: {}",
        engine.wear
    );
    assert!(
        cost > 0.0,
        "a troca forçada do Grave deveria cobrar custo (a débito): {cost}"
    );
}

// -------- Economia do enduro (custo por duração + alívio de parada) --------

/// Um time pobre (sem caixa pra trocar) só ACUMULA desgaste. No enduro (60 min) o desgaste
/// persistido é bem maior que num sprint (30 min); paradas reais aliviam, mas o enduro ainda
/// custa mais. É a conta do enduro fluindo pela economia calibrada, atrás do gate de 40 min.
#[test]
fn enduro_desgasta_mais_o_carro_e_a_parada_alivia() {
    use crate::models::team::placeholder_team_from_db;
    let total_wear = |duracao_min: u16, pits: u32| -> f64 {
        let conn = Connection::open_in_memory().unwrap();
        let mut team = placeholder_team_from_db(
            "T".to_string(),
            "T".to_string(),
            "gt3".to_string(),
            "2026-01-01T00:00:00".to_string(),
        );
        team.cash_balance = 0.0; // sem caixa → não troca, só acumula desgaste
        team.debt_balance = 1e9;
        team.financial_state = "critical".to_string();
        let car = Car::uniform(5);
        team_car::upsert_team_car(&conn, "T", &car).unwrap();
        team.car = Some(car);
        let cond = WearConditions {
            track_pha: (1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0),
            weather: crate::car::breakdown::Weather::NEUTRAL,
            duracao_min,
        };
        // Carro do JOGADOR (Some) com paradas reais; estilo neutro (fator 1.0).
        maintain_team_car_pits(
            &conn,
            &team,
            "gt3",
            1,
            &[],
            cond,
            Some(crate::car::driving_style::StyleFactors::uniform(1.0)),
            true,
            pits,
            &[],
            0,
        )
        .unwrap();
        let after = team_car::get_team_car(&conn, "T").unwrap().unwrap();
        after.parts.iter().map(|p| p.wear).sum()
    };
    let sprint = total_wear(30, 0);
    let enduro = total_wear(60, 0);
    let enduro_pit = total_wear(60, 3); // teto de alívio (−30% do sobrecusto)
    assert!(
        enduro > sprint * 1.5,
        "enduro deveria desgastar bem mais (sprint={sprint:.4} enduro={enduro:.4})"
    );
    assert!(
        enduro_pit < enduro,
        "paradas deveriam aliviar o enduro ({enduro_pit:.4} < {enduro:.4})"
    );
    assert!(
        enduro_pit > sprint,
        "mesmo com paradas o enduro custa mais que o sprint"
    );
}

/// A IA (player_style = None) modela as paradas pela duração — recebe o alívio SOZINHA, sem
/// receber contagem de pit. Enduro da IA custa mais que sprint, mas menos que enduro sem alívio.
#[test]
fn ia_recebe_alivio_modelado_no_enduro() {
    use crate::models::team::placeholder_team_from_db;
    let ai_wear = |duracao_min: u16| -> f64 {
        let conn = Connection::open_in_memory().unwrap();
        let mut team = placeholder_team_from_db(
            "T".to_string(),
            "T".to_string(),
            "gt3".to_string(),
            "2026-01-01T00:00:00".to_string(),
        );
        team.cash_balance = 0.0;
        team.debt_balance = 1e9;
        team.financial_state = "critical".to_string();
        let car = Car::uniform(5);
        team_car::upsert_team_car(&conn, "T", &car).unwrap();
        team.car = Some(car);
        let cond = WearConditions {
            track_pha: (1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0),
            weather: crate::car::breakdown::Weather::NEUTRAL,
            duracao_min,
        };
        maintain_team_car(&conn, &team, "gt3", 1, &[], cond, None).unwrap();
        team_car::get_team_car(&conn, "T")
            .unwrap()
            .unwrap()
            .parts
            .iter()
            .map(|p| p.wear)
            .sum()
    };
    let sprint = ai_wear(30);
    let enduro = ai_wear(60); // 2 paradas modeladas → −20% do sobrecusto
    assert!(
        enduro > sprint,
        "enduro da IA deveria custar mais ({enduro:.4} > {sprint:.4})"
    );
    // Sobrecusto 60min = 1.0; com 2 paradas modeladas (−20%) → mult 1.8 → 1.8× o sprint.
    assert!(
        (enduro / sprint - 1.8).abs() < 0.02,
        "IA 60min deveria ser ~1.8× o sprint, deu {:.3}",
        enduro / sprint
    );
}

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
    const RISK_OPEN: f64 = 0.87; // espelha breakdown::RISK_OPEN

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
        println!("     (peças na zona de risco ≥87%)");

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
            // "Em risco" = a peça CRUZA a zona (≥87%) DURANTE esta corrida (entrada +
            // desgaste da corrida), não só se já entrou acima de 87%.
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
