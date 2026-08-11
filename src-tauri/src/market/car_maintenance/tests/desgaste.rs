//! Desgaste e o que castiga a peça: contato de disputa, quebra, DNF e a conta do enduro.

use super::super::*;
use super::*;
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

