use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::evolution::growth::SeasonStats;
use crate::models::driver::Driver;
use crate::models::enums::PrimaryPersonality;
use crate::simulation::pressure::MOTIVATION_REF;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotivationReport {
    pub driver_id: String,
    pub old_motivation: u8,
    pub new_motivation: u8,
    pub delta: i8,
    pub reasons: Vec<String>,
}

/// Fracao da distancia ate a linha de base que a motivacao percorre de volta a
/// cada virada, ANTES dos deltas do ano.
///
/// Sem ela o ajuste e catraca: os positivos se acumulam, o clamp em 100 guarda o
/// credito e o eixo satura. O harness mediu exatamente isso — media subindo de
/// 65.3 para 84.9 em 15 temporadas, 39% do grid travado no teto e sem plato a
/// vista. Com o decaimento, so continua no teto quem continua ganhando.
const MOTIVATION_DECAY: f64 = 0.15;

/// O que a TEMPORADA disse ao piloto. Disponivel no fim do campeonato, antes de
/// promocao e mercado — ver [`OffseasonContext`] para a outra metade.
#[derive(Debug, Clone, Copy)]
pub struct MotivationContext {
    pub was_champion: bool,
    pub seasons_in_category: i32,
    /// Terminou bem acima do que a forca do carro/equipe previa (carregou a maquina).
    pub outperformed_machinery: bool,
}

/// O que o OFFSEASON disse ao piloto: em que DEGRAU ele foi parar depois da
/// promocao e do mercado.
///
/// Estes sinais existiam desde sempre no [`MotivationContext`], mas chegavam ao
/// pipeline com `false` fixo — nao por desleixo, e por ORDEM: a evolucao dos
/// pilotos roda antes da promocao e antes da pre-temporada, entao no momento do
/// ajuste ninguem tinha sido promovido nem rebaixado ainda. O efeito pratico era
/// grave: o -8 por rebaixamento nunca disparava, e sobrava um conjunto de deltas
/// quase so positivo.
///
/// **Sobraram aqui so os dois sinais que SAO um degrau.** Os outros dois eram
/// EVENTOS, derivados da comparacao entre o assento de antes e o de depois, e essa
/// derivacao so funciona onde o mercado JA rodou — o que no modo jogavel nunca e o
/// caso. Cada um passou a ser aplicado no instante em que acontece: ver
/// [`adjust_contract_renewal_motivation`] e [`adjust_lost_seat_motivation`].
#[derive(Debug, Clone, Copy)]
pub struct OffseasonContext {
    pub was_promoted: bool,
    pub was_relegated: bool,
}

pub fn adjust_end_of_season_motivation(
    driver: &mut Driver,
    stats: &SeasonStats,
    ctx: &MotivationContext,
    _rng: &mut impl Rng,
) -> MotivationReport {
    let old_motivation = driver.motivacao.round().clamp(0.0, 100.0) as u8;

    // O decaimento vem ANTES dos deltas: o ano passado esfria, o ano atual fala.
    driver.motivacao += (MOTIVATION_REF - driver.motivacao) * MOTIVATION_DECAY;

    let mut delta = 0_i32;
    let mut reasons = Vec::new();

    if ctx.was_champion {
        delta += 20;
        reasons.push("Campeao da temporada! (+20)".to_string());
    }
    if ctx.outperformed_machinery {
        delta += 10;
        reasons.push("Superou o carro/equipe (+10)".to_string());
    }
    if stats.posicao_campeonato <= 3 && !ctx.was_champion {
        delta += 8;
        reasons.push("Top 3 no campeonato (+8)".to_string());
    }

    // A tabela pesa DOS DOIS LADOS. Antes, a metade de cima ganhava +3 e a de
    // baixo nao perdia nada — metade do grid recebia credito de graca todo ano,
    // que era o motor principal da catraca.
    let top_half = stats.posicao_campeonato <= (stats.total_pilotos / 2).max(1);
    let bottom_quarter = stats.posicao_campeonato > (stats.total_pilotos * 3 / 4).max(1);
    if top_half && stats.posicao_campeonato > 3 {
        delta += 3;
        reasons.push("Resultado solido (+3)".to_string());
    } else if !top_half {
        if bottom_quarter {
            delta -= 6;
            reasons.push("Fundo da tabela (-6)".to_string());
        } else {
            delta -= 3;
            reasons.push("Metade de baixo da tabela (-3)".to_string());
        }
    }

    if ctx.seasons_in_category >= 3 {
        delta -= 2;
        reasons.push("Estagnacao na mesma categoria (-2)".to_string());
    }
    if stats.dnfs >= 3 {
        delta -= 3;
        reasons.push("Muitos abandonos na temporada (-3)".to_string());
    }

    if matches!(
        driver.personalidade_primaria,
        Some(PrimaryPersonality::Consolidador)
    ) && top_half
    {
        delta += 3;
        reasons.push("Consolidador satisfeito com resultado (+3)".to_string());
    }

    finalizar(driver, old_motivation, delta, reasons)
}

/// Segundo passe: o DEGRAU em que o offseason deixou o piloto, aplicado depois que
/// promocao e mercado ja rodaram.
///
/// **Defasagem conhecida:** o `check_retirement` roda no primeiro passe, entao a
/// queda por cair de categoria so conta para `temporadas_motivacao_baixa` na virada
/// seguinte. O piloto amarga a temporada e so entao desiste — defensavel na
/// ficcao, mas e uma escolha, nao um acidente.
pub fn adjust_offseason_motivation(
    driver: &mut Driver,
    ctx: &OffseasonContext,
) -> MotivationReport {
    let old_motivation = driver.motivacao.round().clamp(0.0, 100.0) as u8;
    let mut delta = 0_i32;
    let mut reasons = Vec::new();

    if ctx.was_promoted {
        delta += 15;
        reasons.push("Promovido de categoria (+15)".to_string());
    }
    if ctx.was_relegated {
        delta -= 8;
        reasons.push("Rebaixado de categoria (-8)".to_string());
    }

    // O ambicioso mede o ano pelo degrau, e o degrau so se conhece aqui.
    if matches!(
        driver.personalidade_primaria,
        Some(PrimaryPersonality::Ambicioso)
    ) && !ctx.was_promoted
        && driver.temporadas_na_categoria >= 2
    {
        delta -= 5;
        reasons.push("Ambicioso frustrado por nao subir (-5)".to_string());
    }

    finalizar(driver, old_motivation, delta, reasons)
}

/// O bonus de motivacao por renovar contrato. E o mesmo +5 de sempre, que morava
/// dentro de [`adjust_offseason_motivation`]; so mudou de gatilho, nao de valor.
const BONUS_CONTRATO_RENOVADO: i32 = 5;
const RAZAO_CONTRATO_RENOVADO: &str = "Contrato renovado (+5)";

/// **+5 pela RENOVACAO, aplicado no instante em que ela acontece.**
///
/// O sinal era derivado no segundo passe da motivacao, comparando o assento de
/// antes da virada com o de depois. A derivacao esta certa, e so vale onde o
/// mercado JA rodou — o caminho historico, em que a pre-temporada inteira e
/// resolvida dentro da propria virada.
///
/// No modo JOGAVEL o mercado nao roda ali: a virada so deixa o plano da
/// pre-temporada em staging, e as pre-passes (que expiram contrato e renovam)
/// caem ao sair da semana 1, ja depois do commit. Quando o segundo passe rodava,
/// nenhuma renovacao existia — o bonus nunca acontecia para ninguem.
///
/// Aqui o gatilho e o FATO, nao a foto: quem chama e a pre-passe que grava o
/// contrato novo (`market::pipeline`). Com isso o bonus vale nos dois modos, uma
/// vez por renovacao, e continua fora do alcance de quem nao renovou nada — quem
/// esta no meio de um plurianual nao tem contrato vencendo e nem entra na lista, e
/// quem troca de equipe assina, nao renova.
pub fn adjust_contract_renewal_motivation(driver: &mut Driver) -> MotivationReport {
    let old_motivation = driver.motivacao.round().clamp(0.0, 100.0) as u8;
    finalizar(
        driver,
        old_motivation,
        BONUS_CONTRATO_RENOVADO,
        vec![RAZAO_CONTRATO_RENOVADO.to_string()],
    )
}

/// A penalidade por perder a vaga. E o mesmo -10 de sempre, que morava dentro de
/// [`adjust_offseason_motivation`]; so mudou de gatilho, nao de valor.
const PENALIDADE_VAGA_PERDIDA: i32 = -10;
const RAZAO_VAGA_PERDIDA: &str = "Perdeu a vaga na equipe (-10)";

/// **-10 pela PERDA DA VAGA, aplicado no instante em que ela acontece.**
///
/// Bug irmao do da renovacao, e pela mesma causa. O sinal era derivado no segundo
/// passe da motivacao — "nao tem assento depois da virada" —, e a derivacao so vale
/// onde o mercado JA rodou dentro da propria virada, que e o caminho historico.
///
/// No modo JOGAVEL, quando o segundo passe rodava o contrato antigo ainda estava de
/// pe: a virada so deixa o plano da pre-temporada em staging, e a pre-passe que
/// expira contrato cai ao sair da semana 1, depois do commit. Ninguem tinha perdido
/// vaga nenhuma ainda, entao o -10 nunca disparava para ninguem.
///
/// Aqui o gatilho e o FATO. Sao tres formas de ficar sem assento, cada uma aplicando o
/// efeito onde ela propria e persistida:
/// 1. o contrato venceu e a equipe nao renovou (`market::pipeline`, pre-passe de
///    contratos) — o caminho de longe mais comum;
/// 2. a equipe subiu de categoria e o piloto nao tinha a licenca, sendo liberado com o
///    contrato rescindido (`promotion::pilots`, efeito `FreedNoLicense`);
/// 3. o assento dele foi dado a outro piloto no meio do contrato, no assedio
///    (`market::pipeline::assedio` entre IAs, `assedio_jogador` quando quem chega e o
///    jogador). Quem perde e o DESLOCADO, nunca o jogador.
///
/// Quem TROCA de equipe nao passa por nenhuma das tres: rebaixamento por merito e o
/// proprio assediado rescindem e assinam no mesmo passo, e quem assina depois com outro
/// time ja perdeu a vaga la atras — o -10 e do momento em que ficou sem assento, nao do
/// balanco do fim da janela. Varredura de integridade (o `sync` de slots, a limpeza de
/// contrato duplicado) tambem nao e evento e nao chama nada disto.
pub fn adjust_lost_seat_motivation(driver: &mut Driver) -> MotivationReport {
    let old_motivation = driver.motivacao.round().clamp(0.0, 100.0) as u8;
    finalizar(
        driver,
        old_motivation,
        PENALIDADE_VAGA_PERDIDA,
        vec![RAZAO_VAGA_PERDIDA.to_string()],
    )
}

fn finalizar(
    driver: &mut Driver,
    old_motivation: u8,
    delta: i32,
    reasons: Vec<String>,
) -> MotivationReport {
    let new_motivation_value = (driver.motivacao + delta as f64).clamp(0.0, 100.0);
    driver.motivacao = new_motivation_value;

    let new_motivation = new_motivation_value.round().clamp(0.0, 100.0) as u8;
    MotivationReport {
        driver_id: driver.id.clone(),
        old_motivation,
        new_motivation,
        delta: new_motivation as i8 - old_motivation as i8,
        reasons,
    }
}

#[cfg(test)]
mod tests {
    use rand::{rngs::StdRng, SeedableRng};

    use super::*;

    #[test]
    fn test_champion_motivation_boost() {
        let mut driver = sample_driver(50.0, None);
        let stats = SeasonStats {
            posicao_campeonato: 1,
            total_pilotos: 20,
            pontos: 120,
            vitorias: 4,
            podios: 6,
            corridas: 8,
            dnfs: 0,
        };
        let mut rng = StdRng::seed_from_u64(1);

        let ctx = MotivationContext {
            was_champion: true,
            seasons_in_category: 1,
            outperformed_machinery: false,
        };
        let report = adjust_end_of_season_motivation(&mut driver, &stats, &ctx, &mut rng);

        assert!(report.delta >= 20);
        assert!(driver.motivacao > 50.0);
    }

    #[test]
    fn test_stagnation_penalty() {
        // Na linha de base (70) o decaimento e neutro, entao o teste mede so os
        // deltas do ano — que e o que ele sempre quis medir.
        let mut driver = sample_driver(MOTIVATION_REF, Some(PrimaryPersonality::Ambicioso));
        let stats = SeasonStats {
            posicao_campeonato: 12,
            total_pilotos: 20,
            pontos: 40,
            vitorias: 0,
            podios: 1,
            corridas: 8,
            dnfs: 1,
        };
        let mut rng = StdRng::seed_from_u64(2);

        let ctx = MotivationContext {
            was_champion: false,
            seasons_in_category: 3,
            outperformed_machinery: false,
        };
        let report = adjust_end_of_season_motivation(&mut driver, &stats, &ctx, &mut rng);

        assert!(report.delta < 0);
        assert!(report
            .reasons
            .iter()
            .any(|reason| reason.contains("Estagnacao")));
    }

    #[test]
    fn test_motivation_clamped_0_100() {
        let stats = SeasonStats {
            posicao_campeonato: 20,
            total_pilotos: 20,
            pontos: 0,
            vitorias: 0,
            podios: 0,
            corridas: 8,
            dnfs: 4,
        };
        // Lanterna que ainda por cima perde a vaga: o passe de fim de temporada mais
        // a perda do assento batem no piso e param nele.
        let mut low_driver = sample_driver(2.0, Some(PrimaryPersonality::Ambicioso));
        low_driver.temporadas_na_categoria = 4;
        let mut rng_low = StdRng::seed_from_u64(3);
        let low_ctx = MotivationContext {
            was_champion: false,
            seasons_in_category: 4,
            outperformed_machinery: false,
        };
        adjust_end_of_season_motivation(&mut low_driver, &stats, &low_ctx, &mut rng_low);
        let low_report = adjust_lost_seat_motivation(&mut low_driver);
        assert_eq!(low_report.new_motivation, 0);

        let mut high_driver = sample_driver(95.0, Some(PrimaryPersonality::Consolidador));
        let mut rng_high = StdRng::seed_from_u64(4);
        let high_ctx = MotivationContext {
            was_champion: true,
            seasons_in_category: 1,
            outperformed_machinery: false,
        };
        let high_report = adjust_end_of_season_motivation(
            &mut high_driver,
            &SeasonStats {
                posicao_campeonato: 1,
                total_pilotos: 20,
                pontos: 120,
                vitorias: 4,
                podios: 7,
                corridas: 8,
                dnfs: 0,
            },
            &high_ctx,
            &mut rng_high,
        );
        assert_eq!(high_report.new_motivation, 100);
    }

    /// O decaimento sozinho: sem nada acontecendo, a motivacao volta em direcao a
    /// linha de base — dos dois lados. E o que impede a catraca de travar no teto.
    #[test]
    fn decaimento_puxa_os_dois_extremos_para_a_base() {
        let stats = SeasonStats {
            posicao_campeonato: 10,
            total_pilotos: 20,
            pontos: 40,
            vitorias: 0,
            podios: 0,
            corridas: 8,
            dnfs: 0,
        };
        let ctx = MotivationContext {
            was_champion: false,
            seasons_in_category: 1,
            outperformed_machinery: false,
        };

        // Meio de tabela pelo lado de cima: so o +3 de resultado solido entra, e o
        // teto perde terreno mesmo assim.
        let mut no_teto = sample_driver(100.0, None);
        let mut rng = StdRng::seed_from_u64(11);
        adjust_end_of_season_motivation(&mut no_teto, &stats, &ctx, &mut rng);
        assert!(
            no_teto.motivacao < 100.0,
            "quem esta no teto sem ganhar nada tem de descer, e desceu para {}",
            no_teto.motivacao
        );

        let mut no_chao = sample_driver(0.0, None);
        let mut rng = StdRng::seed_from_u64(12);
        adjust_end_of_season_motivation(&mut no_chao, &stats, &ctx, &mut rng);
        assert!(no_chao.motivacao > 0.0);
    }

    /// A tabela pesa dos dois lados: o fundo perde mais que o meio de baixo, e os
    /// dois perdem. Antes, a metade de baixo inteira nao perdia nada.
    #[test]
    fn metade_de_baixo_paga_e_o_fundo_paga_mais() {
        let ctx = MotivationContext {
            was_champion: false,
            seasons_in_category: 1,
            outperformed_machinery: false,
        };
        let stats_com = |posicao: i32| SeasonStats {
            posicao_campeonato: posicao,
            total_pilotos: 20,
            pontos: 0,
            vitorias: 0,
            podios: 0,
            corridas: 8,
            dnfs: 0,
        };

        let mut meio_de_baixo = sample_driver(MOTIVATION_REF, None);
        let mut rng = StdRng::seed_from_u64(21);
        let r_meio =
            adjust_end_of_season_motivation(&mut meio_de_baixo, &stats_com(13), &ctx, &mut rng);

        let mut lanterna = sample_driver(MOTIVATION_REF, None);
        let mut rng = StdRng::seed_from_u64(22);
        let r_fundo =
            adjust_end_of_season_motivation(&mut lanterna, &stats_com(19), &ctx, &mut rng);

        assert!(r_meio.delta < 0, "metade de baixo tem de perder motivacao");
        assert!(
            r_fundo.delta < r_meio.delta,
            "o fundo da tabela tem de doer mais que o meio de baixo"
        );
    }

    /// O sinal que o pipeline nunca entregou: perder a vaga derruba a motivacao. Vale
    /// -10 e vale UMA vez — o gatilho mudou de lugar (do segundo passe para o instante
    /// em que o assento se perde), o valor nao.
    #[test]
    fn perder_a_vaga_vale_dez_e_uma_vez_so() {
        let mut driver = sample_driver(MOTIVATION_REF, None);
        let report = adjust_lost_seat_motivation(&mut driver);
        assert_eq!(report.delta, -10);
        assert_eq!(report.reasons.len(), 1);
        assert!(report.reasons[0].contains("Perdeu a vaga"));

        // Chamado duas vezes desconta duas vezes: a garantia de "uma vez por perda" e
        // de quem chama, e mora nos dois call sites do evento — a pre-passe do mercado
        // (que consome o contrato vencido) e a liberacao por falta de licenca.
        let segundo = adjust_lost_seat_motivation(&mut driver);
        assert_eq!(segundo.delta, -10);
    }

    /// O bonus de renovacao continua valendo +5 e continua sendo UM so — o gatilho
    /// mudou de lugar (do segundo passe para o instante da renovacao), o valor nao.
    #[test]
    fn renovacao_vale_cinco_e_uma_vez_so() {
        let mut driver = sample_driver(MOTIVATION_REF, None);
        let report = adjust_contract_renewal_motivation(&mut driver);
        assert_eq!(report.delta, 5);
        assert_eq!(report.reasons.len(), 1);
        assert!(report.reasons[0].contains("Contrato renovado"));

        // Chamado duas vezes soma duas vezes: a garantia de "uma vez por renovacao"
        // e de quem chama, e mora na pre-passe do mercado — que grava um contrato
        // novo por piloto por janela.
        let segundo = adjust_contract_renewal_motivation(&mut driver);
        assert_eq!(segundo.delta, 5);
    }

    /// O segundo passe so fala de DEGRAU: quem muda de categoria nao ganha "Contrato
    /// renovado" de brinde nem leva "Perdeu a vaga" por tabela. Os dois eventos tem
    /// gatilho proprio, no instante em que acontecem.
    #[test]
    fn o_passe_de_offseason_so_fala_de_degrau() {
        for ctx in [
            OffseasonContext {
                was_promoted: true,
                was_relegated: false,
            },
            OffseasonContext {
                was_promoted: false,
                was_relegated: true,
            },
            OffseasonContext {
                was_promoted: false,
                was_relegated: false,
            },
        ] {
            let mut driver = sample_driver(MOTIVATION_REF, None);
            let report = adjust_offseason_motivation(&mut driver, &ctx);
            for proibida in ["Contrato renovado", "Perdeu a vaga"] {
                assert!(
                    !report.reasons.iter().any(|r| r.contains(proibida)),
                    "{ctx:?} falou de '{proibida}': {:?}",
                    report.reasons
                );
            }
        }
    }

    #[test]
    fn test_outperformed_machinery_cushions_motivation() {
        // Bom piloto carregando um carro ruim (terminou acima do esperado): em vez
        // de despencar, ganha reconhecimento e nao some por desmotivacao.
        let mut driver = sample_driver(50.0, Some(PrimaryPersonality::Ambicioso));
        let stats = SeasonStats {
            posicao_campeonato: 12,
            total_pilotos: 20,
            pontos: 30,
            vitorias: 0,
            podios: 0,
            corridas: 8,
            dnfs: 1,
        };
        let mut rng = StdRng::seed_from_u64(7);
        let ctx = MotivationContext {
            was_champion: false,
            seasons_in_category: 3,
            outperformed_machinery: true,
        };
        let report = adjust_end_of_season_motivation(&mut driver, &stats, &ctx, &mut rng);
        assert!(report.reasons.iter().any(|r| r.contains("Superou")));
        assert!(report.delta > 0);
    }

    fn sample_driver(motivation: f64, personality: Option<PrimaryPersonality>) -> Driver {
        let mut driver = Driver::new(
            "P003".to_string(),
            "Piloto Motivado".to_string(),
            "Brasil".to_string(),
            "M".to_string(),
            24,
            2020,
        );
        driver.motivacao = motivation;
        driver.personalidade_primaria = personality;
        driver
    }
}
