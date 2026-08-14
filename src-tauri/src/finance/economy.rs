#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlobalEconomicHealth {
    Boom,
    Neutral,
    Recession,
}

pub fn economy_income_modifier(health: GlobalEconomicHealth) -> f64 {
    match health {
        GlobalEconomicHealth::Boom => 1.12,
        GlobalEconomicHealth::Neutral => 1.0,
        GlobalEconomicHealth::Recession => 0.9,
    }
}

pub fn economy_cost_modifier(health: GlobalEconomicHealth) -> f64 {
    match health {
        GlobalEconomicHealth::Boom => 1.08,
        GlobalEconomicHealth::Neutral => 1.0,
        GlobalEconomicHealth::Recession => 0.92,
    }
}

/// Quantas temporadas tem um ciclo macroeconômico: uma recessão e um boom por ciclo.
const TEMPORADAS_POR_CICLO: i32 = 6;

/// Hash determinístico e ESTÁVEL (FNV-1a de 64 bits).
///
/// Escrito à mão de propósito, em vez de `DefaultHasher`: o resultado disto vira a
/// macroeconomia de um save que dura 25 temporadas, e a `std` não garante que o
/// `DefaultHasher` produza o mesmo número entre versões do Rust. Um save não pode trocar de
/// história econômica porque o toolchain subiu.
fn semente_estavel(career_id: &str, ciclo: i32) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mistura = |h: &mut u64, byte: u8| {
        *h ^= byte as u64;
        *h = h.wrapping_mul(0x100_0000_01b3);
    };
    for byte in career_id.as_bytes() {
        mistura(&mut h, *byte);
    }
    for byte in ciclo.to_le_bytes() {
        mistura(&mut h, byte);
    }
    h
}

/// A saúde macroeconômica de uma temporada **neste save**.
///
/// O ciclo continua sendo o mesmo do desenho original — uma recessão e um boom a cada
/// [`TEMPORADAS_POR_CICLO`] temporadas, o resto neutro —, mas **as posições dentro do ciclo
/// são sorteadas pela identidade do save**. Antes eram fixas (`season % 6`: recessão sempre
/// na múltipla de 6, boom sempre na 3), o que fazia todo jogador viver a mesma sequência e,
/// pior, saber de antemão o ano em que a receita cai.
///
/// A cadência é preservada de propósito: sortear cada temporada de forma independente daria
/// três recessões seguidas em alguns saves e nenhuma em vinte temporadas noutros. O que se
/// quer é o mesmo ritmo com a posição imprevisível.
pub fn global_economic_health_do_save(career_id: &str, season_number: i32) -> GlobalEconomicHealth {
    let ciclo = season_number.div_euclid(TEMPORADAS_POR_CICLO);
    let posicao = season_number.rem_euclid(TEMPORADAS_POR_CICLO);
    let semente = semente_estavel(career_id, ciclo);

    let recessao = (semente % TEMPORADAS_POR_CICLO as u64) as i32;
    // O boom cai numa das posições restantes — nunca na mesma da recessão.
    let restante =
        ((semente / TEMPORADAS_POR_CICLO as u64) % (TEMPORADAS_POR_CICLO - 1) as u64) as i32;
    let boom = if restante >= recessao {
        restante + 1
    } else {
        restante
    };

    if posicao == recessao {
        GlobalEconomicHealth::Recession
    } else if posicao == boom {
        GlobalEconomicHealth::Boom
    } else {
        GlobalEconomicHealth::Neutral
    }
}

/// O ciclo fixo, sem a identidade do save — **só para teste**.
///
/// Recessão em toda temporada múltipla de 6, boom sempre na 3. Era isto que a produção
/// rodava, e é exatamente o que o item A7.10 tirou dela: todo jogador vivia a mesma
/// sequência e sabia de antemão o ano em que a receita cai. Os três call sites vivos
/// (`commands::race::persistencia`, `commands::race::importacao` e a fatura em
/// `commands::race`) passaram a chamar [`global_economic_health_do_save`].
///
/// Sobrevive `#[cfg(test)]` porque os harness de medição financeira querem uma cronologia
/// econômica FIXA entre execuções e entre saves — é o que mantém os números deles
/// comparáveis. A anotação é a trava: nenhum caminho de produção volta a compilar contra
/// ela sem passar por aqui.
#[cfg(test)]
pub fn global_economic_health_for_season(season_number: i32) -> GlobalEconomicHealth {
    match season_number.rem_euclid(TEMPORADAS_POR_CICLO) {
        0 => GlobalEconomicHealth::Recession,
        3 => GlobalEconomicHealth::Boom,
        _ => GlobalEconomicHealth::Neutral,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neutral_economy_modifier_defaults_to_one() {
        assert_eq!(economy_income_modifier(GlobalEconomicHealth::Neutral), 1.0);
    }

    #[test]
    fn recession_reduces_costs_as_well() {
        assert!(economy_cost_modifier(GlobalEconomicHealth::Recession) < 1.0);
    }

    #[test]
    fn boom_and_recession_move_income_in_opposite_directions() {
        assert!(
            economy_income_modifier(GlobalEconomicHealth::Boom)
                > economy_income_modifier(GlobalEconomicHealth::Neutral)
        );
        assert!(
            economy_income_modifier(GlobalEconomicHealth::Recession)
                < economy_income_modifier(GlobalEconomicHealth::Neutral)
        );
    }

    #[test]
    fn season_cycle_produces_booms_and_recessions() {
        assert_eq!(
            global_economic_health_for_season(3),
            GlobalEconomicHealth::Boom
        );
        assert_eq!(
            global_economic_health_for_season(6),
            GlobalEconomicHealth::Recession
        );
        assert_eq!(
            global_economic_health_for_season(7),
            GlobalEconomicHealth::Neutral
        );
    }

    /// **A cadência é a mesma em todo save; a posição, não.** É o contrato inteiro do
    /// sorteio: cada bloco de 6 temporadas tem exatamente uma recessão e um boom, em
    /// posições que dependem de qual carreira é.
    #[test]
    fn cada_ciclo_tem_uma_recessao_e_um_boom_em_qualquer_save() {
        for save in ["carreira-a", "carreira-b", "", "9f0c7b1e-uuid-de-save"] {
            for ciclo in -2..=8 {
                let inicio = ciclo * TEMPORADAS_POR_CICLO;
                let saudes: Vec<_> = (inicio..inicio + TEMPORADAS_POR_CICLO)
                    .map(|t| global_economic_health_do_save(save, t))
                    .collect();
                assert_eq!(
                    saudes
                        .iter()
                        .filter(|s| **s == GlobalEconomicHealth::Recession)
                        .count(),
                    1,
                    "save {save:?} ciclo {ciclo}: {saudes:?}"
                );
                assert_eq!(
                    saudes
                        .iter()
                        .filter(|s| **s == GlobalEconomicHealth::Boom)
                        .count(),
                    1,
                    "save {save:?} ciclo {ciclo}: {saudes:?}"
                );
            }
        }
    }

    /// **Só a posição mudou; a frequência, não.** Num horizonte alinhado ao ciclo, o sorteio
    /// por save entrega exatamente o mesmo número de recessões e de booms que a cronologia
    /// fixa antiga. É a trava contra o risco real de trocar a função nos call sites de
    /// produção: mexer sem querer em quanto o mundo passa em recessão seria rebalancear a
    /// economia inteira por acidente.
    #[test]
    fn a_cadencia_e_a_mesma_da_cronologia_fixa() {
        const HORIZONTE: i32 = 5 * TEMPORADAS_POR_CICLO;
        let conta = |saudes: &[GlobalEconomicHealth], alvo: GlobalEconomicHealth| {
            saudes.iter().filter(|s| **s == alvo).count()
        };
        let legado: Vec<_> = (0..HORIZONTE)
            .map(global_economic_health_for_season)
            .collect();
        for save in ["carreira-a", "carreira-b", "", "9f0c7b1e-uuid-de-save"] {
            let por_save: Vec<_> = (0..HORIZONTE)
                .map(|t| global_economic_health_do_save(save, t))
                .collect();
            for alvo in [
                GlobalEconomicHealth::Recession,
                GlobalEconomicHealth::Boom,
                GlobalEconomicHealth::Neutral,
            ] {
                assert_eq!(
                    conta(&por_save, alvo),
                    conta(&legado, alvo),
                    "save {save:?} mudou a frequência de {alvo:?}: {por_save:?}"
                );
            }
        }
    }

    /// Dois saves diferentes vivem histórias diferentes — o ponto do item.
    #[test]
    fn saves_diferentes_nao_vivem_a_mesma_sequencia() {
        let sequencia = |save: &str| -> Vec<GlobalEconomicHealth> {
            (1..=24)
                .map(|t| global_economic_health_do_save(save, t))
                .collect()
        };
        let a = sequencia("carreira-a");
        assert!(
            ["carreira-b", "carreira-c", "carreira-d"]
                .iter()
                .any(|outro| sequencia(outro) != a),
            "todos os saves caíram na mesma sequência"
        );
    }

    /// Determinístico: o mesmo save responde o mesmo para sempre. Sem isto, reabrir um save
    /// reescreveria a macroeconomia das temporadas já jogadas.
    #[test]
    fn o_mesmo_save_responde_sempre_o_mesmo() {
        for t in -12..=40 {
            assert_eq!(
                global_economic_health_do_save("save-x", t),
                global_economic_health_do_save("save-x", t)
            );
        }
        // E o hash é o nosso, escrito à mão, justamente para não depender do toolchain.
        assert_eq!(semente_estavel("", 0), 5_558_979_605_539_197_941);
    }
}
