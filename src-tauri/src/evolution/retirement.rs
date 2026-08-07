use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::models::driver::Driver;
use crate::models::enums::DriverStatus;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetirementResult {
    pub should_retire: bool,
    pub reason: Option<String>,
}

pub fn check_retirement(
    driver: &Driver,
    consecutive_low_motivation_seasons: i32,
    has_severe_injury: bool,
    rng: &mut impl Rng,
) -> RetirementResult {
    if has_severe_injury && rng.gen::<f64>() < 0.40 {
        return RetirementResult {
            should_retire: true,
            reason: Some("Aposentou-se devido a lesao grave".to_string()),
        };
    }

    let age = driver.idade;
    let skill = driver.atributos.skill;

    // Desânimo aposenta — mas TALENTO COMPRA PACIÊNCIA, e antes não comprava. Este era o único
    // ramo da função sem termo de skill: as duas temporadas valiam igual para o craque e para o
    // pilotão, enquanto a aposentadoria por idade e a do órfão ocioso já pesavam talento.
    //
    // Por que isso importa agora: a motivação SEGUE O RESULTADO, então este ramo é o caminho pelo
    // qual uma má fase vira fim de carreira. Com a variância antiga um bom piloto quase nunca
    // emendava duas temporadas ruins; com as camadas de evento calibradas, emenda — e as
    // aposentadorias do mundo histórico saltaram de 518 para 684, secando a oferta de veteranos
    // que a escada precisa para preencher o topo.
    //
    // A regra é o que o automobilismo faz de verdade: quem tem talento recebe outro assento e não
    // pendura o capacete na primeira sequência ruim; quem não tem, sai. Um fracasso deixa de ser
    // terminal para quem tem com o que voltar.
    // A IDADE compra paciência junto com o talento, e os dois SOMAM porque são
    // motivos independentes: talento dá alternativa, juventude dá tempo. O ramo
    // não olhava idade nenhuma — um piloto de 22 anos pendurava o capacete no
    // mesmo prazo de um de 38 —, e o harness mostrou o tamanho do buraco: das 123
    // desistências por desmotivação em 5 carreiras × 15 temporadas, 44 (36%) eram
    // de pilotos com menos de 29 anos, e a faixa 25–28, o auge de uma carreira,
    // era a MAIOR de todas com 24%.
    //
    // Veterano desistir fecha; jovem desistir não. Quem tem carreira inteira pela
    // frente insiste — e quando não vinga, sai pela porta que já existe para isso
    // (o órfão ocioso, mais abaixo), não por desânimo.
    let paciencia_por_talento = if skill >= 70.0 {
        4
    } else if skill >= 55.0 {
        3
    } else {
        2
    };
    let paciencia_por_idade = match age {
        ..=23 => 3,
        24..=27 => 2,
        28..=31 => 1,
        _ => 0,
    };
    let temporadas_para_desistir = paciencia_por_talento + paciencia_por_idade;
    if driver.motivacao < 20.0 && consecutive_low_motivation_seasons >= temporadas_para_desistir {
        return RetirementResult {
            should_retire: true,
            reason: Some("Aposentou-se por falta de motivacao".to_string()),
        };
    }

    // Pilotos da IA que nunca competiram e já passaram da idade de estreia
    // dificilmente entrarão num grid — aposentam cedo para não se acumularem
    // como agentes livres eternos (órfãos que inflavam o mundo sem nunca correr).
    if !driver.is_jogador && driver.stats_carreira.corridas == 0 {
        let never_raced_chance = match age {
            27..=29 => 0.40,
            30..=32 => 0.70,
            33.. => 0.95,
            _ => 0.0,
        };
        if never_raced_chance > 0.0 && rng.gen::<f64>() < never_raced_chance {
            return RetirementResult {
                should_retire: true,
                reason: Some(format!("Aposentou-se aos {age} anos sem nunca competir")),
            };
        }
    }

    let chance = match age {
        36..=37 => {
            if skill < 35.0 {
                0.30
            } else {
                0.05
            }
        }
        38 => {
            if skill < 40.0 {
                0.35
            } else {
                0.15
            }
        }
        39 => 0.20,
        40 => 0.30,
        41 => 0.40,
        42 => 0.50,
        43 => 0.60,
        44 => 0.70,
        45 => 0.85,
        46 => 0.95,
        47.. => 1.00,
        _ => 0.0,
    };

    if chance > 0.0 && rng.gen::<f64>() < chance {
        return RetirementResult {
            should_retire: true,
            reason: Some(format!("Aposentou-se aos {} anos", age)),
        };
    }

    RetirementResult {
        should_retire: false,
        reason: None,
    }
}

pub fn process_retirement(driver: &mut Driver) {
    driver.status = DriverStatus::Aposentado;
}

/// Chance de um órfão OCIOSO aposentar (item 6). Órfão ocioso = IA sem assento que
/// NÃO correu na temporada: o mercado teve uma janela inteira e não o contratou.
/// Para não acumular como agente livre eterno (o que alimentava os resgates de
/// última hora sem piso), tem chance de pendurar o capacete — alta para
/// fracos/veteranos, baixa para os bons. Uma jovem promessa (≤21 e skill ≥55) é
/// preservada (ainda pode ser resgatada na próxima janela): chance 0.
pub fn idle_orphan_retirement_chance(age: u32, skill: f64) -> f64 {
    // NINGUÉM desiste da carreira aos 22 anos por não ter achado vaga — é cedo
    // demais, e sem assento não há o que cansar. A isenção antiga (≤21 E skill
    // ≥55) quase nunca alcançava esta população: o harness mediu o pool de livres
    // com idade média entre 19,5 e 23,9 e overall entre 40 e 51, ou seja, jovens
    // DEMAIS para desistir e fracos DEMAIS para serem protegidos pelo talento.
    //
    // Não vira agente livre eterno porque a isenção só ADIA: o piloto envelhece
    // dentro do pool e volta a rolar o dado ao cruzar a faixa.
    if age <= 22 {
        return 0.0;
    }
    let base: f64 = if skill < 45.0 {
        0.55
    } else if skill < 60.0 {
        0.30
    } else {
        0.12
    };
    let age_bonus: f64 = if age >= 33 {
        0.25
    } else if age >= 28 {
        0.12
    } else {
        0.0
    };
    let bruto = (base + age_bonus).min(0.95);
    // Dos 23 aos 27 ainda se insiste: a chance existe, mas pela metade. O teto de
    // 40 agentes livres do `closed_system_playable_world_has_no_orphans_and_drivers_raced`
    // é o que impede afrouxar mais que isto — no draft histórico (25 anos) sobra
    // bem menos folga que no mundo jogável.
    if age <= 27 {
        bruto * 0.5
    } else {
        bruto
    }
}

/// Chance de uma lesão GRAVE encerrar a carreira no meio da temporada. Base baixa
/// (~10% num grid típico), ponderada por idade: um jovem quase nunca pendura o capacete
/// por uma fratura; um veterano tem bem mais chance. Só IA — o piloto do jogador nunca é
/// aposentado à força (checado no chamador).
pub fn severe_injury_retirement_chance(age: u32) -> f64 {
    match age {
        0..=22 => 0.03,
        23..=27 => 0.06,
        28..=32 => 0.10,
        33..=36 => 0.16,
        37..=40 => 0.24,
        _ => 0.35,
    }
}

#[cfg(test)]
mod tests {
    use rand::{rngs::StdRng, SeedableRng};

    use super::*;

    #[test]
    fn test_no_retirement_young() {
        let driver = sample_driver(24, 60.0, 80.0);
        let mut rng = StdRng::seed_from_u64(1);

        let result = check_retirement(&driver, 0, false, &mut rng);

        assert!(!result.should_retire);
        assert!(result.reason.is_none());
    }

    #[test]
    fn test_guaranteed_retirement_47_plus() {
        let driver = sample_driver(47, 60.0, 60.0);
        let mut rng = StdRng::seed_from_u64(2);

        let result = check_retirement(&driver, 0, false, &mut rng);

        assert!(result.should_retire);
        assert_eq!(result.reason.as_deref(), Some("Aposentou-se aos 47 anos"));
    }

    #[test]
    fn test_low_motivation_retirement() {
        let mut rng = StdRng::seed_from_u64(3);
        // Idade 33: fora de qualquer faixa de aposentadoria por idade (que começa aos 36) E
        // fora da paciência por juventude (que zera aos 32), então o único desfecho possível
        // aqui é o desânimo puro — o que isola o ramo sob teste.
        let fraco = sample_driver(33, 40.0, 10.0);

        let result = check_retirement(&fraco, 2, false, &mut rng);

        assert!(result.should_retire);
        assert_eq!(
            result.reason.as_deref(),
            Some("Aposentou-se por falta de motivacao")
        );
    }

    /// **Talento compra paciência.** O que este teste guarda não é um limiar, é a ordem: um
    /// piloto melhor tem que aguentar mais temporadas ruins antes de pendurar o capacete.
    ///
    /// Antes, este ramo era o único da função sem termo de skill, e o craque saía no mesmo prazo
    /// do pilotão. Como a motivação segue o resultado, isso fazia de uma má fase um fim de
    /// carreira — e quanto mais variância a simulação ganha, mais bons pilotos ela descartava.
    #[test]
    fn desanimo_descarta_o_fraco_antes_do_craque() {
        let mut rng = StdRng::seed_from_u64(7);
        // Todos aos 33: idade onde a paciência por juventude já zerou, isolando o eixo do
        // talento — que é o que este teste guarda.
        let craque = sample_driver(33, 80.0, 10.0);
        let mediano = sample_driver(33, 60.0, 10.0);
        let fraco = sample_driver(33, 40.0, 10.0);

        // Duas temporadas ruins: só o fraco desiste.
        assert!(check_retirement(&fraco, 2, false, &mut rng).should_retire);
        assert!(!check_retirement(&mediano, 2, false, &mut rng).should_retire);
        assert!(!check_retirement(&craque, 2, false, &mut rng).should_retire);

        // Três: o mediano acompanha; o craque ainda tem crédito.
        assert!(check_retirement(&mediano, 3, false, &mut rng).should_retire);
        assert!(!check_retirement(&craque, 3, false, &mut rng).should_retire);

        // Quatro: nem o talento segura mais.
        assert!(check_retirement(&craque, 4, false, &mut rng).should_retire);
    }

    /// **Juventude compra paciência**, pelo mesmo motivo que o talento compra: quem
    /// tem carreira pela frente insiste. Mesmo skill, idades diferentes — o que o
    /// teste guarda é a ordem, não os limiares.
    ///
    /// O ramo de desânimo não olhava idade nenhuma, e o harness mediu o resultado:
    /// 36% das desistências por desmotivação eram de pilotos com menos de 29 anos,
    /// com a faixa 25–28 sendo a maior de todas.
    #[test]
    fn desanimo_descarta_o_veterano_antes_do_jovem() {
        let mut rng = StdRng::seed_from_u64(11);
        let jovem = sample_driver(22, 40.0, 10.0);
        let em_ascensao = sample_driver(26, 40.0, 10.0);
        let maduro = sample_driver(30, 40.0, 10.0);
        let veterano = sample_driver(34, 40.0, 10.0);

        // Duas temporadas ruins: só quem já não tem tempo pela frente desiste.
        assert!(check_retirement(&veterano, 2, false, &mut rng).should_retire);
        assert!(!check_retirement(&maduro, 2, false, &mut rng).should_retire);
        assert!(!check_retirement(&em_ascensao, 2, false, &mut rng).should_retire);
        assert!(!check_retirement(&jovem, 2, false, &mut rng).should_retire);

        // O jovem aguenta mais que todos, e por margem larga.
        assert!(check_retirement(&maduro, 3, false, &mut rng).should_retire);
        assert!(check_retirement(&em_ascensao, 4, false, &mut rng).should_retire);
        assert!(!check_retirement(&jovem, 4, false, &mut rng).should_retire);
        assert!(check_retirement(&jovem, 5, false, &mut rng).should_retire);
    }

    fn sample_driver(age: u32, skill: f64, motivation: f64) -> Driver {
        let mut driver = Driver::new(
            "P004".to_string(),
            "Piloto Veteranissimo".to_string(),
            "Brasil".to_string(),
            "M".to_string(),
            age,
            2020,
        );
        driver.atributos.skill = skill;
        driver.motivacao = motivation;
        // Por padrão o piloto de teste já competiu na carreira (caso comum).
        driver.stats_carreira.corridas = 100;
        driver
    }

    #[test]
    fn test_never_raced_ai_retires_far_more_than_a_raced_peer() {
        let mut never_raced_retired = 0;
        let mut raced_retired = 0;

        for seed in 0..200 {
            let raced = sample_driver(31, 60.0, 80.0);
            let mut rng = StdRng::seed_from_u64(seed);
            if check_retirement(&raced, 0, false, &mut rng).should_retire {
                raced_retired += 1;
            }

            let mut never_raced = sample_driver(31, 60.0, 80.0);
            never_raced.stats_carreira.corridas = 0;
            let mut rng = StdRng::seed_from_u64(seed);
            if check_retirement(&never_raced, 0, false, &mut rng).should_retire {
                never_raced_retired += 1;
            }
        }

        // Aos 31 o veterano que correu nao tem chance por idade (0%), enquanto o
        // que nunca correu deve aposentar com folga (~70%).
        assert_eq!(raced_retired, 0);
        assert!(never_raced_retired > 100);
    }

    #[test]
    fn test_never_raced_player_is_never_force_retired() {
        for seed in 0..100 {
            let mut player = sample_driver(33, 60.0, 80.0);
            player.is_jogador = true;
            player.stats_carreira.corridas = 0;
            let mut rng = StdRng::seed_from_u64(seed);

            assert!(
                !check_retirement(&player, 0, false, &mut rng).should_retire,
                "jogador nunca deve ser aposentado pela regra de nunca-correu (seed {seed})"
            );
        }
    }

    #[test]
    fn test_severe_injury_retirement_chance_increases_with_age() {
        assert!(
            severe_injury_retirement_chance(20) < severe_injury_retirement_chance(30),
            "jovem deve ter menos chance que piloto de 30"
        );
        assert!(severe_injury_retirement_chance(30) < severe_injury_retirement_chance(38));
        assert!(severe_injury_retirement_chance(38) < severe_injury_retirement_chance(45));
        // Jovem: bem raro. Veterano: elevado, mas nunca uma certeza.
        assert!(severe_injury_retirement_chance(20) <= 0.05);
        assert!(severe_injury_retirement_chance(45) < 1.0);
    }

    #[test]
    fn test_idle_orphan_retirement_scales_and_protects_young_talent() {
        // Até os 22 ninguém desiste por falta de vaga, INCLUSIVE o fraco. A proteção
        // antiga exigia skill ≥55 e por isso não alcançava quem de fato fica livre:
        // o pool medido tem idade média ~21 e overall ~45.
        assert_eq!(idle_orphan_retirement_chance(20, 70.0), 0.0);
        assert_eq!(idle_orphan_retirement_chance(22, 30.0), 0.0);
        // Fraco aposenta muito mais que um bom piloto na mesma idade.
        assert!(idle_orphan_retirement_chance(25, 30.0) > idle_orphan_retirement_chance(25, 75.0));
        // A idade aumenta a chance para o mesmo skill.
        assert!(idle_orphan_retirement_chance(35, 40.0) > idle_orphan_retirement_chance(24, 40.0));
        // Dos 23 aos 27 ainda se insiste: metade da chance que o mesmo perfil teria
        // depois dos 28. A isenção ADIA, não anula — por isso o pool não vira eterno.
        let jovem = idle_orphan_retirement_chance(25, 30.0);
        let maduro = idle_orphan_retirement_chance(29, 30.0);
        assert!(jovem > 0.0, "dos 23 aos 27 a chance existe");
        assert!(
            jovem < maduro * 0.75,
            "insistir aos 25 tem de custar bem menos que aos 29: {jovem} vs {maduro}"
        );
    }

    #[test]
    fn test_never_raced_young_ai_is_not_retired() {
        for seed in 0..100 {
            let mut rookie = sample_driver(24, 60.0, 80.0);
            rookie.stats_carreira.corridas = 0;
            let mut rng = StdRng::seed_from_u64(seed);

            assert!(!check_retirement(&rookie, 0, false, &mut rng).should_retire);
        }
    }
}
