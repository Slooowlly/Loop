//! O offseason de competitividade — **mecanismo aposentado, vivo só em teste**.
//!
//! Este era o passo de pré-temporada que credita confiabilidade, engenharia e instalações a
//! partir do caixa, da dívida, do estado financeiro e da estratégia da equipe. Ele **saiu de
//! produção**: quem ocupa a linha dele em `market::preseason::inicializacao` é
//! [`crate::economia::desenvolvimento`], onde a equipe INVESTE o excedente de caixa e recebe
//! estrutura em troca, com retorno decrescente e depreciação.
//!
//! O motivo da troca está medido: aqui a estrutura subia ~2,76 pontos por equipe por
//! temporada **sem debitar nada**. Era uma das fontes de dinheiro do nada do redesign de
//! economia, e a razão declarada de o superávit das equipes não ter para onde ir — nenhum
//! débito escalava com a riqueza, então o caixa integrava para sempre. A varredura do harness
//! (`varrer_ralo`) mediu o modelo novo em 9/9 categorias dentro do alvo de deriva de caixa,
//! com crise em 12,2%.
//!
//! **Por que o código fica, em vez de sumir:** ele é o BRAÇO DE CONTROLE do harness A/B de
//! economia (`Offseason::Producao` em `commands::race::tests::medicao_financeira`), que roda
//! os dois modelos lado a lado. Apagá-lo apagaria a base de comparação de toda a calibração
//! do módulo novo. O `#[cfg(test)]` no `mod` é o que garante que ele não volte a rodar por
//! descuido: um caller de produção novo passa a não compilar.
//!
//! **Consequência para a dívida de unidade:** os divisores em dólar absoluto daqui
//! (`cash_balance / 1_000_000`, `debt_balance / 900_000`) são a razão de a força de caixa
//! saturar na categoria rica e nunca ligar na base — o mesmo caixa vale coisas diferentes em
//! divisões de porte diferente. Reancorá-los em meses de operação
//! ([`crate::economia::temporada::meses_de_operacao`]) era a correção pendente; ela deixou de
//! ser dívida ativa quando o mecanismo saiu de produção, e agora **não deve ser feita**: o
//! valor deste código é ser o retrato fiel do modelo antigo contra o qual o novo foi medido.
//! Mexer nos divisores invalidaria a comparação sem melhorar nada que o jogador veja.

use crate::finance::events::technical_breakthrough_chance;
use crate::finance::focus::TeamFocus;
use crate::models::team::Team;

/// O que uma offseason move nos atributos da equipe.
///
/// `car_performance_delta` é calculado e devolvido, mas **não escreve em coluna nenhuma**:
/// quem constrói carro é o Sistema de Nível do Carro, na tabela `team_car`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OffseasonCompetitivenessImpact {
    pub reliability_delta: f64,
    pub car_performance_delta: f64,
    pub engineering_delta: f64,
    pub facilities_delta: f64,
}

/// Pilar B: controla o quão rápido os retornos do investimento no carro decaem.
/// Quanto maior o `car_performance` atual, menos cada unidade de "drive" rende
/// (subir o carro fica progressivamente mais caro) — sem teto, só mais lento.
const CAR_INVEST_DIMINISH_K: f64 = 14.0;

/// Fração do ganho de carro que as equipes FICTÍCIAS (sem marca de fábrica) obtêm
/// quando disputam contra fábricas reais (GT3 e GT3 do endurance). As reais ganham
/// 100%; as fictícias ganham menos, então temporada a temporada as fábricas reais se
/// distanciam — deixando as fictícias como escolhas de "azarão". Ajustável.
const FICTIONAL_CAR_DEV_FACTOR: f64 = 0.6;

/// Em quantos anos de CARREIRA a penalidade fictícia some por completo. A penalidade
/// é forte no começo (azarão recém-chegado) e desaparece linearmente, deixando o
/// jogador "underdog" capaz de competir de igual para igual ao final do período.
pub(crate) const PENALTY_FADE_YEARS: f64 = 5.0;

/// Quanto cada equipe multiplica seu GANHO de car_performance nesta offseason.
/// É 1.0 para fábricas reais e para qualquer categoria sem disputa real-vs-fictícia;
/// nas arenas GT3 (sprint) e GT3-endurance as fictícias rendem `FICTIONAL_CAR_DEV_FACTOR`
/// no ano 0 da carreira, subindo linearmente até 1.0 no ano `PENALTY_FADE_YEARS`.
/// `career_year` é o ano da carreira do jogador (0 = primeiro ano).
fn car_dev_gain_factor(team: &Team, career_year: i32) -> f64 {
    let competes_with_real_marques = team.categoria == "gt3"
        || (team.categoria == "endurance" && team.classe.as_deref() == Some("gt3"));
    if team.marca.is_none() && competes_with_real_marques {
        let fade = (career_year.max(0) as f64 / PENALTY_FADE_YEARS).min(1.0);
        FICTIONAL_CAR_DEV_FACTOR + (1.0 - FICTIONAL_CAR_DEV_FACTOR) * fade
    } else {
        1.0
    }
}

/// Multiplicador do GANHO de car_performance no offseason segundo o FOCO da equipe
/// (ideia 4). É a consequência real do foco no carro: um time de projeto de longo
/// prazo canaliza mais verba para o desenvolvimento; um time em sobrevivência corta
/// custo; um celeiro de talentos investe nos pilotos/base, não no carro. Neutro
/// (1.0) para meio de grid / reconstrução. Só modula o ganho (drive > 0), igual ao
/// `car_dev_gain_factor` — quedas por falta de verba aplicam cheias.
fn focus_car_dev_factor(focus: TeamFocus) -> f64 {
    match focus {
        TeamFocus::Dinastia => 1.20,
        TeamFocus::ProjetoDeTitulo => 1.12,
        TeamFocus::MeioDeGrid | TeamFocus::Reconstrucao => 1.0,
        TeamFocus::Celeiro => 0.90,
        TeamFocus::Sobrevivencia => 0.82,
    }
}

/// `career_year` é o ano da carreira do jogador (0 = primeiro ano). Modula a
/// penalidade fictícia GT3, que some por completo ao atingir `PENALTY_FADE_YEARS`.
/// `focus` é o foco vigente da equipe (ideia 4): modula o GANHO no carro.
pub fn calculate_offseason_competitiveness_impact(
    team: &Team,
    career_year: i32,
    focus: TeamFocus,
) -> OffseasonCompetitivenessImpact {
    let efficiency = management_efficiency_modifier(team);
    // Teto de caixa elevado (1.2 -> 2.5): equipes realmente ricas investem mais
    // no carro (Pilar B). Os retornos decrescentes abaixo é que limitam o ganho.
    let cash_strength = (team.cash_balance / 1_000_000.0).clamp(-0.5, 2.5);
    // Pilar B: o investimento no CARRO escala com o caixa numa faixa muito maior
    // que o cash_strength geral (teto 2.5). Assim o caixa gigante das categorias
    // de topo (endurance ~12-60M) vira carro de verdade — gating economico em vez
    // de teto rigido. Confiabilidade/estrutura seguem no cash_strength original.
    let car_cash_strength = (team.cash_balance / 1_000_000.0).clamp(-0.5, 12.0);
    let debt_pressure = (team.debt_balance / 900_000.0).clamp(0.0, 1.2);
    let state = financial_state_bias(&team.financial_state);
    let strategy = season_strategy_bias(&team.season_strategy);
    let breakthrough_expected_value = technical_breakthrough_chance(team) * 4.0;

    let reliability_delta =
        (cash_strength * 1.8 - debt_pressure * 3.2 + state.reliability + strategy.reliability)
            * efficiency;
    // "Drive" bruto de investimento no carro nesta offseason.
    let car_drive = (car_cash_strength * 0.55 - debt_pressure * 0.65
        + state.car_performance
        + strategy.car_performance
        + breakthrough_expected_value)
        * efficiency;
    // Retornos decrescentes (Pilar B): o mesmo drive rende menos quanto melhor o
    // carro já é. Quedas (drive < 0, carro defasado por falta de verba) aplicam
    // direto, para que elites defundadas ainda percam terreno. O fator real-vs-
    // fictícia só penaliza o GANHO (não as quedas), para as fábricas se distanciarem.
    let car_performance_delta = if car_drive > 0.0 {
        car_drive * car_dev_gain_factor(team, career_year) * focus_car_dev_factor(focus)
            / (1.0 + team.car_performance.max(0.0) / CAR_INVEST_DIMINISH_K)
    } else {
        car_drive
    };
    let structure_delta =
        (cash_strength * 1.15 - debt_pressure * 2.25 + state.structure + strategy.structure)
            * efficiency;

    OffseasonCompetitivenessImpact {
        reliability_delta: reliability_delta.clamp(-6.0, 4.0),
        // Clamp largo (de ±1.4) só como rede de segurança contra saltos extremos;
        // os retornos decrescentes é que regulam o ganho temporada a temporada.
        car_performance_delta: car_performance_delta.clamp(-3.0, 3.0),
        engineering_delta: structure_delta.clamp(-3.5, 2.5),
        facilities_delta: (structure_delta * 0.75).clamp(-2.5, 1.8),
    }
}

/// `career_year` é o ano da carreira do jogador (0 = primeiro ano); é repassado a
/// `calculate_offseason_competitiveness_impact` para modular a penalidade fictícia GT3.
/// `focus` é o foco vigente da equipe (ideia 4): modula o GANHO no carro.
pub fn apply_offseason_competitiveness_impact(
    team: &mut Team,
    career_year: i32,
    focus: TeamFocus,
) -> OffseasonCompetitivenessImpact {
    let impact = calculate_offseason_competitiveness_impact(team, career_year, focus);

    team.confiabilidade = (team.confiabilidade + impact.reliability_delta).clamp(0.0, 100.0);
    // O CARRO não se move mais por aqui. Quem constrói carro é o Sistema de Nível do Carro:
    // o cérebro de manutenção decide compra/upgrade a cada corrida, olhando o caixa real, e
    // o resultado vive em `team_car` — que é o que `effective_car_performance` lê.
    //
    // Mexer também na coluna legada era investir duas vezes no mesmo carro e, pior, num
    // número que ninguém lê pra ritmo e que não tem teto: 26 temporadas de offseason
    // levavam uma equipe a ~5× o topo do domínio (−5..16) sem que nada na tela mudasse,
    // porque `car_strength` satura em 100. O `car_performance_delta` continua sendo
    // calculado e devolvido — o relatório de pré-temporada mostra a INTENÇÃO de investimento
    // da equipe —, mas não escreve mais na coluna.
    team.engineering = (team.engineering + impact.engineering_delta).clamp(0.0, 100.0);
    team.facilities = (team.facilities + impact.facilities_delta).clamp(0.0, 100.0);

    impact
}

fn management_efficiency_modifier(team: &Team) -> f64 {
    let morale_score = ((team.morale - 0.5) * 100.0).clamp(0.0, 100.0);
    let raw_efficiency = team.engineering * 0.40
        + team.facilities * 0.25
        + morale_score * 0.20
        + team.reputacao * 0.15;

    0.75 + (raw_efficiency.clamp(0.0, 100.0) / 100.0) * 0.50
}

#[derive(Debug, Clone, Copy)]
struct FinanceBias {
    reliability: f64,
    car_performance: f64,
    structure: f64,
}

/// Viés nulo. É o que uma chave DESCONHECIDA recebe — e desconhecida aqui significa save
/// de uma versão futura ou coluna corrompida, não um estado novo do jogo: os estados vivos
/// estão todos nomeados em [`EstadoFinanceiro`], e o teste
/// `todo_estado_produzido_por_state_rs_tem_bias` trava a lista contra quem os produz.
const BIAS_NEUTRO: FinanceBias = FinanceBias {
    reliability: 0.0,
    car_performance: 0.0,
    structure: 0.0,
};

/// As seis faixas de saúde financeira, como [`crate::finance::state`] as escreve na coluna.
///
/// Existe para fechar um contrato que era só texto: o `match &str` daqui tinha braço `_`, e
/// **"stable" nunca teve braço próprio** — a faixa central da escada caía no catch-all. O
/// efeito (viés zero) é o mesmo, e continua sendo, de propósito: a mudança aqui é que agora
/// isso é uma decisão escrita, e renomear uma faixa em `state.rs` quebra um teste em vez de
/// zerar o efeito em silêncio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EstadoFinanceiro {
    Elite,
    Saudavel,
    Estavel,
    Pressionado,
    Crise,
    Colapso,
}

impl EstadoFinanceiro {
    /// Resolve a chave persistida. `None` = chave que este build não conhece.
    fn from_key(key: &str) -> Option<Self> {
        match key {
            "elite" => Some(Self::Elite),
            "healthy" => Some(Self::Saudavel),
            "stable" => Some(Self::Estavel),
            "pressured" => Some(Self::Pressionado),
            "crisis" => Some(Self::Crise),
            "collapse" => Some(Self::Colapso),
            _ => None,
        }
    }
}

/// As estratégias de temporada, como [`crate::finance::strategy`] e o resgate as escrevem.
///
/// Mesma história do estado: **"balanced" é o default de toda equipe nova**
/// (`models::team`) e não tinha braço próprio — caía num catch-all que, ao contrário do
/// estado, NÃO era neutro (0,15/0,15/0,05). Os números continuam idênticos; o que muda é
/// que o default do jogo deixou de depender de um braço de fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EstrategiaDaTemporada {
    Equilibrada,
    Expansao,
    Austeridade,
    TudoOuNada,
    DominioDeElite,
    Sobrevivencia,
}

impl EstrategiaDaTemporada {
    fn from_key(key: &str) -> Option<Self> {
        match key {
            "balanced" => Some(Self::Equilibrada),
            "expansion" => Some(Self::Expansao),
            "austerity" => Some(Self::Austeridade),
            "all_in" => Some(Self::TudoOuNada),
            "elite_dominance" => Some(Self::DominioDeElite),
            "survival" => Some(Self::Sobrevivencia),
            _ => None,
        }
    }
}

fn financial_state_bias(state: &str) -> FinanceBias {
    let Some(estado) = EstadoFinanceiro::from_key(state) else {
        return BIAS_NEUTRO;
    };
    match estado {
        EstadoFinanceiro::Elite => FinanceBias {
            reliability: 0.9,
            car_performance: 0.45,
            structure: 0.75,
        },
        EstadoFinanceiro::Saudavel => FinanceBias {
            reliability: 0.55,
            car_performance: 0.25,
            structure: 0.45,
        },
        // A faixa central não move nada, e é assim desde sempre — antes por cair no braço
        // `_`, agora por escolha declarada. É o ponto de equilíbrio entre `healthy` (+) e
        // `pressured` (−).
        EstadoFinanceiro::Estavel => BIAS_NEUTRO,
        EstadoFinanceiro::Pressionado => FinanceBias {
            reliability: -0.55,
            car_performance: 0.05,
            structure: -0.35,
        },
        EstadoFinanceiro::Crise => FinanceBias {
            reliability: -1.25,
            car_performance: -0.25,
            structure: -0.95,
        },
        EstadoFinanceiro::Colapso => FinanceBias {
            reliability: -2.25,
            car_performance: -0.65,
            structure: -1.85,
        },
    }
}

/// Viés da estratégia EQUILIBRADA — o default de toda equipe nova. Era o braço `_` da
/// função, o que fazia o caminho mais percorrido do jogo depender de um fallback.
const BIAS_EQUILIBRADA: FinanceBias = FinanceBias {
    reliability: 0.15,
    car_performance: 0.15,
    structure: 0.05,
};

fn season_strategy_bias(strategy: &str) -> FinanceBias {
    // Chave desconhecida cai na equilibrada, que é o que o antigo `_` já fazia — trocar
    // isto por neutro mudaria o comportamento de saves com estratégia não reconhecida.
    let Some(estrategia) = EstrategiaDaTemporada::from_key(strategy) else {
        return BIAS_EQUILIBRADA;
    };
    match estrategia {
        EstrategiaDaTemporada::Equilibrada => BIAS_EQUILIBRADA,
        EstrategiaDaTemporada::Expansao => FinanceBias {
            reliability: 0.15,
            car_performance: 0.55,
            structure: 0.55,
        },
        EstrategiaDaTemporada::Austeridade => FinanceBias {
            reliability: 0.2,
            car_performance: -0.25,
            structure: -0.15,
        },
        EstrategiaDaTemporada::TudoOuNada => FinanceBias {
            reliability: -0.8,
            car_performance: 0.95,
            structure: -0.45,
        },
        // Pilar D: dinastia de elite. Carro forte (nível all_in) SEM sacrificar
        // confiabilidade/estrutura — sustentado pelo piso de recursos, não por aposta.
        // O piso é quem separa as 3 elites do meio do grid; a agressividade fica
        // contida (≈ all_in) para o título REVEZAR entre as 3 (sustos), sem uma só
        // dominar tudo.
        EstrategiaDaTemporada::DominioDeElite => FinanceBias {
            reliability: 0.2,
            car_performance: 0.95,
            structure: 0.4,
        },
        EstrategiaDaTemporada::Sobrevivencia => FinanceBias {
            reliability: -0.45,
            car_performance: -0.55,
            structure: -0.85,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::team::placeholder_team_from_db;

    fn sample_team(id: &str, cash: f64, debt: f64, state: &str, strategy: &str) -> Team {
        let mut team = placeholder_team_from_db(
            id.to_string(),
            "Equipe Financeira".to_string(),
            "gt3".to_string(),
            "2026-01-01".to_string(),
        );
        team.cash_balance = cash;
        team.debt_balance = debt;
        team.financial_state = state.to_string();
        team.season_strategy = strategy.to_string();
        team.budget = 55.0;
        team.engineering = 60.0;
        team.facilities = 58.0;
        team.reputacao = 52.0;
        team.morale = 1.0;
        team.confiabilidade = 70.0;
        team.car_performance = 8.0;
        team
    }

    /// **Toda chave que o jogo produz tem braço próprio aqui.** O contrato entre
    /// `finance::state`/`finance::strategy` (que escrevem a coluna) e o viés que a lê era só
    /// texto, com um braço `_` cobrindo o que sobrasse — e sobrava a faixa "stable" e a
    /// estratégia "balanced", que é o default de toda equipe nova.
    ///
    /// O teste varre os produtores em vez de repetir a lista à mão: renomear uma faixa em
    /// `state.rs` quebra aqui, e não em silêncio no balanceamento.
    #[test]
    fn todo_estado_e_estrategia_produzido_pelo_jogo_tem_bias_proprio() {
        use crate::finance::state::{derive_financial_state, estado_por_meses, FaixasDeMeses};
        use crate::finance::strategy::season_strategy_from_plan;

        let faixas = FaixasDeMeses::default();
        for passo in -100..=600 {
            let meses = passo as f64 / 10.0;
            let chave = estado_por_meses(meses, faixas);
            assert!(
                EstadoFinanceiro::from_key(chave).is_some(),
                "estado \"{chave}\" (meses {meses}) não tem braço em financial_state_bias"
            );
        }
        for passo in 0..=1000 {
            let chave = derive_financial_state(passo as f64 / 10.0);
            assert!(
                EstadoFinanceiro::from_key(chave).is_some(),
                "estado \"{chave}\" não tem braço em financial_state_bias"
            );
        }
        // Os planos estratégicos vivos, mais o "austerity" que `finance::rescue` carimba na
        // equipe vendida e o "balanced" que toda equipe nasce com.
        for plano in [
            "sustainable",
            "title_push",
            "rebuild",
            "elite_dominance",
            "desconhecido",
        ] {
            for anos in 0..=4 {
                let chave = season_strategy_from_plan(plano, anos);
                assert!(
                    EstrategiaDaTemporada::from_key(chave).is_some(),
                    "estratégia \"{chave}\" (plano {plano}, {anos} anos) não tem braço em season_strategy_bias"
                );
            }
        }
        for chave in ["austerity", "balanced"] {
            assert!(EstrategiaDaTemporada::from_key(chave).is_some());
        }
    }

    /// O refactor de string para enum não podia mover número nenhum. Estes são os dois
    /// pontos em que o comportamento antigo vinha do braço `_`.
    #[test]
    fn o_fallback_antigo_continua_valendo_onde_valia() {
        // "stable" caía no catch-all neutro e continua neutro.
        let estavel = financial_state_bias("stable");
        assert_eq!(estavel.reliability, 0.0);
        assert_eq!(estavel.car_performance, 0.0);
        assert_eq!(estavel.structure, 0.0);
        // "balanced" caía no catch-all NÃO neutro e continua com os mesmos números.
        let equilibrada = season_strategy_bias("balanced");
        assert_eq!(equilibrada.reliability, 0.15);
        assert_eq!(equilibrada.car_performance, 0.15);
        assert_eq!(equilibrada.structure, 0.05);
        // Chave que este build não conhece: estado vira neutro, estratégia vira equilibrada.
        let desconhecido = financial_state_bias("chave_de_versao_futura");
        assert_eq!(desconhecido.structure, 0.0);
        assert_eq!(
            season_strategy_bias("chave_de_versao_futura").car_performance,
            0.15
        );
    }

    #[test]
    fn real_gt3_marque_outdevelops_an_identical_fictional_team() {
        // Same well-funded GT3 team; only the brand differs. The real manufacturer
        // (marca Some) gains more car performance per offseason than the fictional
        // one (marca None), so factory teams pull away season over season while the
        // fictional teams stay "underdogs".
        let fictional = sample_team("FIC", 1_500_000.0, 0.0, "healthy", "competitive");
        let mut real = sample_team("REAL", 1_500_000.0, 0.0, "healthy", "competitive");
        real.marca = Some("Ferrari".to_string());

        let fictional_gain = calculate_offseason_competitiveness_impact(
            &fictional,
            0,
            crate::finance::focus::TeamFocus::MeioDeGrid,
        )
        .car_performance_delta;
        let real_gain = calculate_offseason_competitiveness_impact(
            &real,
            0,
            crate::finance::focus::TeamFocus::MeioDeGrid,
        )
        .car_performance_delta;

        assert!(
            fictional_gain > 0.0,
            "both teams are investing the car upward"
        );
        assert!(
            real_gain > fictional_gain,
            "real marque ({real_gain}) must out-develop the fictional team ({fictional_gain})"
        );
        // The fictional gain is exactly the real gain scaled by the penalty factor.
        assert!((fictional_gain - real_gain * FICTIONAL_CAR_DEV_FACTOR).abs() < 1e-9);
    }

    #[test]
    fn the_gt3_penalty_does_not_touch_real_marques_or_other_categories() {
        // A fictional team OUTSIDE the real-vs-fictional GT3 arenas keeps full growth.
        let mut amador = sample_team("AMA", 1_500_000.0, 0.0, "healthy", "competitive");
        amador.categoria = "mazda_amador".to_string();
        assert!((car_dev_gain_factor(&amador, 0) - 1.0).abs() < 1e-9);

        // A real marque in GT3-endurance is never penalised.
        let mut real_endurance = sample_team("RE", 1_500_000.0, 0.0, "healthy", "competitive");
        real_endurance.categoria = "endurance".to_string();
        real_endurance.classe = Some("gt3".to_string());
        real_endurance.marca = Some("Audi".to_string());
        assert!((car_dev_gain_factor(&real_endurance, 0) - 1.0).abs() < 1e-9);

        // ...but a fictional GT3-endurance team is.
        let mut fic_endurance = sample_team("FE", 1_500_000.0, 0.0, "healthy", "competitive");
        fic_endurance.categoria = "endurance".to_string();
        fic_endurance.classe = Some("gt3".to_string());
        assert!((car_dev_gain_factor(&fic_endurance, 0) - FICTIONAL_CAR_DEV_FACTOR).abs() < 1e-9);
    }

    #[test]
    fn fictional_gt3_penalty_fades_out_after_five_career_years() {
        // Identical fictional vs real GT3 pair. Early in the career the fictional
        // team develops slower; by career year 5 (and beyond) the penalty is gone
        // and both develop the car at the same rate.
        let fictional = sample_team("FIC", 1_500_000.0, 0.0, "healthy", "competitive");
        let mut real = sample_team("REAL", 1_500_000.0, 0.0, "healthy", "competitive");
        real.marca = Some("Ferrari".to_string());

        // Year 0: full penalty — fictional develops less than the real marque.
        let fictional_y0 = calculate_offseason_competitiveness_impact(
            &fictional,
            0,
            crate::finance::focus::TeamFocus::MeioDeGrid,
        )
        .car_performance_delta;
        let real_y0 = calculate_offseason_competitiveness_impact(
            &real,
            0,
            crate::finance::focus::TeamFocus::MeioDeGrid,
        )
        .car_performance_delta;
        assert!(
            fictional_y0 < real_y0,
            "year 0: fictional ({fictional_y0}) must develop less than real ({real_y0})"
        );

        // Year 5 and 10: penalty fully faded — gains are identical.
        let fictional_y5 = calculate_offseason_competitiveness_impact(
            &fictional,
            5,
            crate::finance::focus::TeamFocus::MeioDeGrid,
        )
        .car_performance_delta;
        let real_y5 = calculate_offseason_competitiveness_impact(
            &real,
            5,
            crate::finance::focus::TeamFocus::MeioDeGrid,
        )
        .car_performance_delta;
        assert!(
            (fictional_y5 - real_y5).abs() < 1e-9,
            "year 5: penalty gone, fictional ({fictional_y5}) == real ({real_y5})"
        );

        let fictional_y10 = calculate_offseason_competitiveness_impact(
            &fictional,
            10,
            crate::finance::focus::TeamFocus::MeioDeGrid,
        )
        .car_performance_delta;
        let real_y10 = calculate_offseason_competitiveness_impact(
            &real,
            10,
            crate::finance::focus::TeamFocus::MeioDeGrid,
        )
        .car_performance_delta;
        assert!(
            (fictional_y10 - real_y10).abs() < 1e-9,
            "year 10: penalty gone, fictional ({fictional_y10}) == real ({real_y10})"
        );
    }

    #[test]
    fn finance_impact_rewards_healthy_cash_with_reliability_support() {
        let rich = sample_team("T001", 1_500_000.0, 0.0, "healthy", "balanced");
        let poor = sample_team("T002", -100_000.0, 650_000.0, "crisis", "survival");

        let rich_impact = calculate_offseason_competitiveness_impact(
            &rich,
            10,
            crate::finance::focus::TeamFocus::MeioDeGrid,
        );
        let poor_impact = calculate_offseason_competitiveness_impact(
            &poor,
            10,
            crate::finance::focus::TeamFocus::MeioDeGrid,
        );

        assert!(rich_impact.reliability_delta > poor_impact.reliability_delta);
        assert!(poor_impact.reliability_delta < 0.0);
    }

    #[test]
    fn finance_impact_gives_all_in_more_car_project_variance_than_balanced() {
        let balanced = sample_team("T001", 600_000.0, 0.0, "stable", "balanced");
        let all_in = sample_team("T002", 600_000.0, 0.0, "pressured", "all_in");

        let balanced_impact = calculate_offseason_competitiveness_impact(
            &balanced,
            10,
            crate::finance::focus::TeamFocus::MeioDeGrid,
        );
        let all_in_impact = calculate_offseason_competitiveness_impact(
            &all_in,
            10,
            crate::finance::focus::TeamFocus::MeioDeGrid,
        );

        assert!(all_in_impact.car_performance_delta > balanced_impact.car_performance_delta);
        assert!(all_in_impact.reliability_delta < balanced_impact.reliability_delta);
    }

    #[test]
    fn applying_finance_impact_changes_team_attributes_with_safe_clamps() {
        let mut team = sample_team("T001", -100_000.0, 900_000.0, "collapse", "survival");
        team.confiabilidade = 4.0;
        team.car_performance = -4.8;
        team.engineering = 2.0;
        team.facilities = 2.0;

        apply_offseason_competitiveness_impact(
            &mut team,
            10,
            crate::finance::focus::TeamFocus::MeioDeGrid,
        );

        assert!((0.0..=100.0).contains(&team.confiabilidade));
        // Pilar B: piso em −5, sem teto superior.
        assert!(team.car_performance >= -5.0);
        assert!((0.0..=100.0).contains(&team.engineering));
        assert!((0.0..=100.0).contains(&team.facilities));
    }

    #[test]
    fn car_investment_has_diminishing_returns() {
        // Mesma força financeira; o carro já alto ganha menos que o carro baixo.
        let mut low = sample_team("T001", 5_000_000.0, 0.0, "elite", "balanced");
        low.car_performance = 2.0;
        let mut high = sample_team("T002", 5_000_000.0, 0.0, "elite", "balanced");
        high.car_performance = 24.0;

        let low_gain = calculate_offseason_competitiveness_impact(
            &low,
            10,
            crate::finance::focus::TeamFocus::MeioDeGrid,
        )
        .car_performance_delta;
        let high_gain = calculate_offseason_competitiveness_impact(
            &high,
            10,
            crate::finance::focus::TeamFocus::MeioDeGrid,
        )
        .car_performance_delta;

        assert!(low_gain > 0.0 && high_gain > 0.0);
        assert!(
            low_gain > high_gain,
            "retornos decrescentes: carro baixo ({low_gain:.3}) deveria ganhar mais que carro alto ({high_gain:.3})"
        );
    }

    #[test]
    fn offseason_nao_move_mais_a_coluna_legada_de_carro() {
        // O carro é construído pelo Sistema de Nível do Carro (peças em `team_car`,
        // decididas corrida a corrida pelo cérebro de manutenção). O offseason NÃO
        // escreve mais em `car_performance`.
        //
        // Este teste substitui o antigo `rich_team_car_grows_past_old_ceiling_of_16`,
        // que garantia o oposto: que uma equipe rica ultrapassasse o topo do domínio.
        // Sem teto e com 26 offseasons de backstory, aquilo levava uma equipe a ~5× o
        // máximo da escala — invisível na tela (`car_strength` satura em 100) e decisivo
        // nas categorias que ainda liam a coluna.
        let mut team = sample_team("T001", 30_000_000.0, 0.0, "elite", "expansion");
        team.car_performance = 15.0;
        for _ in 0..8 {
            apply_offseason_competitiveness_impact(
                &mut team,
                10,
                crate::finance::focus::TeamFocus::MeioDeGrid,
            );
        }
        assert_eq!(
            team.car_performance, 15.0,
            "a coluna legada deve ficar congelada; quem constrói carro é `team_car`"
        );
    }

    #[test]
    fn offseason_ainda_move_confiabilidade_e_estrutura() {
        // O que o Sistema de Nível do Carro NÃO cobre continua vindo daqui.
        let mut team = sample_team("T001", 30_000_000.0, 0.0, "elite", "expansion");
        let reliability_before = team.confiabilidade;
        let engineering_before = team.engineering;

        apply_offseason_competitiveness_impact(
            &mut team,
            10,
            crate::finance::focus::TeamFocus::MeioDeGrid,
        );

        assert!(team.confiabilidade > reliability_before);
        assert!(team.engineering > engineering_before);
    }
}
