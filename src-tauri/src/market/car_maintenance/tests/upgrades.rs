//! A escolha do upgrade: qual peça sobe, quando a janela permite subir e quanto a cota
//! deixa subir de uma vez.

use super::super::*;
use super::*;
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

