use crate::finance::events::technical_breakthrough_chance;
use crate::finance::focus::TeamFocus;
use crate::finance::planning::category_finance_scale;
use crate::models::team::Team;

/// Fração do caixa excedente (acima da reserva de segurança) destinada a abater
/// o principal da dívida a cada corrida.
const DEBT_AMORTIZATION_RATE: f64 = 0.25;
/// Reserva de segurança mantida antes de amortizar, como fração do custo
/// operacional médio da categoria (equipes maiores guardam mais caixa).
const DEBT_AMORTIZATION_RESERVE_FACTOR: f64 = 0.05;

/// Caixa mínimo que a equipe mantém antes de usar a folga para pagar dívida.
fn debt_amortization_reserve(category: &str) -> f64 {
    category_finance_scale(category).operating_cost_midpoint() * DEBT_AMORTIZATION_RESERVE_FACTOR
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoundCashflowSummary {
    pub income: f64,
    pub expenses: f64,
    pub net: f64,
}

/// As OITO linhas de despesa que ficam gravadas no ledger.
///
/// É a mesma forma que a fatura visível (`economia::fatura`) trava por decisão de design:
/// sete linhas físicas da etapa mais o rateio da estrutura do ano. Elas não são um resumo
/// do que saiu do caixa — elas **são** o que saiu do caixa, e é por isso que moram dentro
/// de [`TeamRoundFinanceContext`] em vez de serem recalculadas na hora de gravar. Um
/// segundo cálculo é um segundo lugar onde a conta pode divergir, e o defeito que este
/// redesign existe para corrigir era exatamente esse.
///
/// `viagem` e `estadia` do modelo interno entram somadas porque são a mesma decisão —
/// mandar N pessoas para longe por M noites — e a tela as mostra como uma linha só.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LinhasDaDespesa {
    pub combustivel: f64,
    pub pneus: f64,
    /// Revisão mecânica amortizada por quilômetro. **Não** é a compra de peça de
    /// reposição, que é decisão do cérebro de manutenção e vive em
    /// `technical_investment_cost`.
    pub desgaste_de_peca: f64,
    pub frete: f64,
    pub viagem_e_estadia: f64,
    pub inscricao: f64,
    pub diarias: f64,
    /// A fatia desta rodada nos recorrentes do ano — folha técnica, sede, frota, seguro e
    /// os contratos categóricos. **Sem** a folha de pilotos: ela sai do caixa como
    /// `salary_expense`, e somá-la aqui pagaria piloto duas vezes.
    pub estrutura: f64,
}

impl LinhasDaDespesa {
    /// As sete linhas da ETAPA. É o `event_operations_cost`.
    pub fn total_da_etapa(&self) -> f64 {
        self.combustivel
            + self.pneus
            + self.desgaste_de_peca
            + self.frete
            + self.viagem_e_estadia
            + self.inscricao
            + self.diarias
    }

    /// As oito linhas.
    pub fn total(&self) -> f64 {
        self.total_da_etapa() + self.estrutura
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TeamRoundFinanceContext {
    pub sponsorship_income: f64,
    /// Bilheteria/portão da rodada (Fase 3 do Estrelato): público que a fama do
    /// lineup atrai, escalado pelo prestígio do evento. Canal distinto do patrocínio
    /// (portão volátil e por-evento vs. patrocínio suave e de-temporada).
    pub gate_income: f64,
    pub result_bonus: f64,
    pub partial_prize_income: f64,
    pub aid_income: f64,
    pub salary_expense: f64,
    pub event_operations_cost: f64,
    pub structural_maintenance_cost: f64,
    pub technical_investment_cost: f64,
    pub debt_service_cost: f64,
    /// A decomposição de `event_operations_cost` + `structural_maintenance_cost` em linhas
    /// nomeadas. Os dois agregados acima continuam existindo porque o resto do jogo os lê,
    /// mas são a SOMA destas linhas, escrita de uma origem só — ver
    /// `commands::race::despesa`.
    pub linhas: LinhasDaDespesa,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OffseasonCompetitivenessImpact {
    pub reliability_delta: f64,
    pub car_performance_delta: f64,
    pub engineering_delta: f64,
    pub facilities_delta: f64,
}

/// Fração do custo operacional médio da rodada que vira o BOLO de bilheteria de um
/// evento de prestígio médio. Botão único da magnitude da 2ª receita de fama —
/// calibrado por Monte Carlo. Alvo: bilheteria média por rodada MENOR que o
/// patrocínio (fama já ganha por 2 canais; não pode dominar a economia).
/// Sobrescrevível por `IRACER_GATE_SHARE` (lido uma vez) para varreduras de MC.
const DEFAULT_GATE_POT_COEFF: f64 = 0.12;

/// Peso do PISO de público (a parte da multidão que vem pela corrida, dividida em
/// partes iguais entre os times) vs. o PRÊMIO de estrela (dividido por cota de fama).
/// 0.5 = metade piso, metade estrela. Garante que um time sem astro não zere a
/// bilheteria, e que o estrelato seja um prêmio SOBRE um piso, não tudo-ou-nada.
pub(crate) const GATE_FLOOR_WEIGHT: f64 = 0.5;

pub(crate) fn gate_pot_coeff() -> f64 {
    static COEFF: std::sync::LazyLock<f64> = std::sync::LazyLock::new(|| {
        std::env::var("IRACER_GATE_SHARE")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|v| *v > 0.0 && *v < 1.0)
            .unwrap_or(DEFAULT_GATE_POT_COEFF)
    });
    *COEFF
}

/// Bilheteria da rodada para UM time (Fase 3 do Estrelato). Motor puro e testável.
///
/// - `event_prestige_score`: score do `event_interest` do evento (pré-vendido / esperado).
///   Um evento maior (endurance, final, decisão de título) lota mais → bolo maior.
/// - `round_operating_base`: custo operacional médio da rodada (escala da categoria).
/// - `team_presence`: presença pública do lineup DESTE time (fama dos pilotos).
/// - `grid_total_presence`: soma da presença de TODOS os lineups do grid da categoria.
/// - `n_teams`: nº de times no grid (para o piso dividido em partes iguais).
/// - `income_modifier`: modulador macroeconômico (mesmo das outras receitas).
///
/// `fame_share` = cota competitiva do time no público (presença dele / grid). Se o grid
/// inteiro é anônimo (`grid_total_presence == 0`), cai para cota igual (1/N) — o piso
/// sozinho. Um astro num time pobre puxa portão mesmo sem vencer; o ídolo vale mais se
/// os rivais forem anônimos.
/// Cota de um time no PÚBLICO/bilheteria do evento: piso (dividido igual entre os N
/// times, a multidão que vem pela corrida) + prêmio de estrela (por fama do lineup).
/// Fração ∈ [0,1] do bolo do portão que o time captura. Reusada pela bilheteria e pela
/// prévia da Sala de Estratégia ("sua estrela puxa Y% do público"). Se o grid é anônimo
/// (`grid_total_presence == 0`), cai para cota igual (só o piso).
pub fn team_gate_share(team_presence: f64, grid_total_presence: f64, n_teams: f64) -> f64 {
    team_gate_share_with(
        team_presence,
        grid_total_presence,
        n_teams,
        GATE_FLOOR_WEIGHT,
    )
}

/// Igual a [`team_gate_share`], com o peso do piso explícito. Existe para o harness de
/// calibração varrer o quanto do portão é piso igualitário e o quanto é prêmio de estrela.
pub fn team_gate_share_with(
    team_presence: f64,
    grid_total_presence: f64,
    n_teams: f64,
    floor_weight: f64,
) -> f64 {
    let n = n_teams.max(1.0);
    let piso = floor_weight.clamp(0.0, 1.0);
    let fame_share = if grid_total_presence > 0.0 {
        (team_presence / grid_total_presence).clamp(0.0, 1.0)
    } else {
        1.0 / n
    };
    (piso / n + (1.0 - piso) * fame_share).clamp(0.0, 1.0)
}

pub fn calculate_gate_income(
    event_prestige_score: f64,
    round_operating_base: f64,
    team_presence: f64,
    grid_total_presence: f64,
    n_teams: f64,
    income_modifier: f64,
) -> f64 {
    calculate_gate_income_with(
        event_prestige_score,
        round_operating_base,
        team_presence,
        grid_total_presence,
        n_teams,
        income_modifier,
        gate_pot_coeff(),
        GATE_FLOOR_WEIGHT,
    )
}

/// Igual a [`calculate_gate_income`], com o coeficiente do bolo e o peso do piso explícitos.
///
/// Existe porque `gate_pot_coeff()` é um `LazyLock` — lê a env UMA vez por processo, o que
/// serve para uma varredura de fora mas não para o harness comparar dez valores no mesmo
/// processo. Produção segue na constante.
#[allow(clippy::too_many_arguments)]
pub fn calculate_gate_income_with(
    event_prestige_score: f64,
    round_operating_base: f64,
    team_presence: f64,
    grid_total_presence: f64,
    n_teams: f64,
    income_modifier: f64,
    pot_coeff: f64,
    floor_weight: f64,
) -> f64 {
    // 60 ≈ evento GT3 médio → fator 1.0; clamp evita bolo negativo/absurdo.
    let prestige_factor = (event_prestige_score / 60.0).clamp(0.3, 2.2);
    let gate_pot = round_operating_base * pot_coeff * prestige_factor * income_modifier;

    (gate_pot
        * team_gate_share_with(team_presence, grid_total_presence, n_teams, floor_weight))
    .max(0.0)
}

pub fn calculate_round_income(
    sponsorship_income: f64,
    gate_income: f64,
    result_bonus: f64,
    partial_prize_income: f64,
    aid_income: f64,
) -> f64 {
    sponsorship_income.max(0.0)
        + gate_income.max(0.0)
        + result_bonus.max(0.0)
        + partial_prize_income.max(0.0)
        + aid_income.max(0.0)
}

pub fn calculate_round_expenses(
    salary_expense: f64,
    event_operations_cost: f64,
    structural_maintenance_cost: f64,
    technical_investment_cost: f64,
    debt_service_cost: f64,
) -> f64 {
    salary_expense.max(0.0)
        + event_operations_cost.max(0.0)
        + structural_maintenance_cost.max(0.0)
        + technical_investment_cost.max(0.0)
        + debt_service_cost.max(0.0)
}

pub fn summarize_round_cashflow(income: f64, expenses: f64) -> RoundCashflowSummary {
    RoundCashflowSummary {
        income,
        expenses,
        net: income - expenses,
    }
}

pub fn apply_round_cashflow(
    team: &mut Team,
    context: TeamRoundFinanceContext,
) -> RoundCashflowSummary {
    let income = calculate_round_income(
        context.sponsorship_income,
        context.gate_income,
        context.result_bonus,
        context.partial_prize_income,
        context.aid_income,
    );
    let expenses = calculate_round_expenses(
        context.salary_expense,
        context.event_operations_cost,
        context.structural_maintenance_cost,
        context.technical_investment_cost,
        context.debt_service_cost,
    );
    let summary = summarize_round_cashflow(income, expenses);

    team.last_round_income = summary.income;
    team.last_round_expenses = summary.expenses;
    team.last_round_net = summary.net;
    team.cash_balance += summary.net;

    if team.cash_balance < -100_000.0 {
        let financed_amount = -100_000.0 - team.cash_balance;
        team.debt_balance += financed_amount;
        team.cash_balance = -100_000.0;
    }

    // Amortização: com dívida pendente e caixa acima da reserva de segurança,
    // parte do excedente abate o principal. É o espelho do financiamento acima
    // e o único caminho de recuperação para equipes endividadas — sem isso, a
    // dívida só cresce e "collapse" vira estado absorvente.
    if team.debt_balance > 0.0 {
        let reserve = debt_amortization_reserve(&team.categoria);
        let surplus = team.cash_balance - reserve;
        if surplus > 0.0 {
            let payment = (surplus * DEBT_AMORTIZATION_RATE).min(team.debt_balance);
            team.cash_balance -= payment;
            team.debt_balance -= payment;
        }
    }

    if team.parachute_payment_remaining > 0.0 {
        team.parachute_payment_remaining =
            (team.parachute_payment_remaining - context.aid_income.max(0.0)).max(0.0);
    }

    summary
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

fn financial_state_bias(state: &str) -> FinanceBias {
    match state {
        "elite" => FinanceBias {
            reliability: 0.9,
            car_performance: 0.45,
            structure: 0.75,
        },
        "healthy" => FinanceBias {
            reliability: 0.55,
            car_performance: 0.25,
            structure: 0.45,
        },
        "pressured" => FinanceBias {
            reliability: -0.55,
            car_performance: 0.05,
            structure: -0.35,
        },
        "crisis" => FinanceBias {
            reliability: -1.25,
            car_performance: -0.25,
            structure: -0.95,
        },
        "collapse" => FinanceBias {
            reliability: -2.25,
            car_performance: -0.65,
            structure: -1.85,
        },
        _ => FinanceBias {
            reliability: 0.0,
            car_performance: 0.0,
            structure: 0.0,
        },
    }
}

fn season_strategy_bias(strategy: &str) -> FinanceBias {
    match strategy {
        "expansion" => FinanceBias {
            reliability: 0.15,
            car_performance: 0.55,
            structure: 0.55,
        },
        "austerity" => FinanceBias {
            reliability: 0.2,
            car_performance: -0.25,
            structure: -0.15,
        },
        "all_in" => FinanceBias {
            reliability: -0.8,
            car_performance: 0.95,
            structure: -0.45,
        },
        // Pilar D: dinastia de elite. Carro forte (nível all_in) SEM sacrificar
        // confiabilidade/estrutura — sustentado pelo piso de recursos, não por aposta.
        // O piso é quem separa as 3 elites do meio do grid; a agressividade fica
        // contida (≈ all_in) para o título REVEZAR entre as 3 (sustos), sem uma só
        // dominar tudo.
        "elite_dominance" => FinanceBias {
            reliability: 0.2,
            car_performance: 0.95,
            structure: 0.4,
        },
        "survival" => FinanceBias {
            reliability: -0.45,
            car_performance: -0.55,
            structure: -0.85,
        },
        _ => FinanceBias {
            reliability: 0.15,
            car_performance: 0.15,
            structure: 0.05,
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
    fn round_income_stays_positive_for_basic_team_revenue() {
        let round_income = calculate_round_income(125_000.0, 12_000.0, 25_000.0, 8_000.0, 0.0);
        assert!(round_income > 0.0);
    }

    #[test]
    fn gate_income_sobe_com_a_fama_do_lineup() {
        // Mesmo evento, mesmo grid: o time com lineup mais famoso leva a maior fatia.
        let famoso = calculate_gate_income(60.0, 500_000.0, 160.0, 300.0, 8.0, 1.0);
        let anonimo = calculate_gate_income(60.0, 500_000.0, 40.0, 300.0, 8.0, 1.0);
        assert!(
            famoso > anonimo,
            "lineup famoso ({famoso}) deveria puxar mais bilheteria que o anônimo ({anonimo})"
        );
    }

    #[test]
    fn gate_income_sobe_com_o_prestigio_do_evento() {
        // Mesmo time/grid: evento de prestígio (endurance/final) lota mais que rodada fraca.
        let grande = calculate_gate_income(95.0, 500_000.0, 100.0, 300.0, 8.0, 1.0);
        let pequeno = calculate_gate_income(25.0, 500_000.0, 100.0, 300.0, 8.0, 1.0);
        assert!(
            grande > pequeno,
            "evento grande ({grande}) > pequeno ({pequeno})"
        );
    }

    #[test]
    fn team_gate_share_soma_piso_mais_premio_de_estrela() {
        // Estrela (presença 200/400) leva mais que anônimo (50/400); ambos ∈ (0,1).
        let estrela = team_gate_share(200.0, 400.0, 8.0);
        let anonimo = team_gate_share(50.0, 400.0, 8.0);
        assert!(estrela > anonimo);
        assert!(estrela < 1.0 && anonimo > 0.0);
        // Grid anônimo → só o piso (cota igual 1/N).
        let piso = team_gate_share(0.0, 0.0, 8.0);
        assert!((piso - 1.0 / 8.0).abs() < 1e-9);
    }

    #[test]
    fn gate_income_tem_piso_mesmo_sem_estrela() {
        // Grid inteiro anônimo (presença total 0): ainda há bilheteria (piso, cota igual).
        let sem_fama = calculate_gate_income(60.0, 500_000.0, 0.0, 0.0, 8.0, 1.0);
        assert!(
            sem_fama > 0.0,
            "piso de público deveria garantir bilheteria > 0"
        );
    }

    #[test]
    fn round_expenses_stay_positive_for_basic_team_costs() {
        let round_expenses =
            calculate_round_expenses(60_000.0, 22_000.0, 15_000.0, 9_500.0, 3_000.0);
        assert!(round_expenses > 0.0);
    }

    #[test]
    fn round_cashflow_summary_tracks_net_value() {
        let summary = summarize_round_cashflow(158_000.0, 121_500.0);

        assert_eq!(summary.net, 36_500.0);
    }

    #[test]
    fn apply_round_cashflow_updates_team_snapshot() {
        let mut team = placeholder_team_from_db(
            "T001".to_string(),
            "Equipe Financeira".to_string(),
            "gt3".to_string(),
            "2026-01-01".to_string(),
        );
        team.cash_balance = 500_000.0;

        let summary = apply_round_cashflow(
            &mut team,
            TeamRoundFinanceContext {
                linhas: Default::default(),
                sponsorship_income: 120_000.0,
                gate_income: 0.0,
                result_bonus: 25_000.0,
                partial_prize_income: 10_000.0,
                aid_income: 0.0,
                salary_expense: 45_000.0,
                event_operations_cost: 20_000.0,
                structural_maintenance_cost: 15_000.0,
                technical_investment_cost: 18_000.0,
                debt_service_cost: 2_500.0,
                ..Default::default()
            },
        );

        assert_eq!(team.last_round_income, summary.income);
        assert_eq!(team.last_round_expenses, summary.expenses);
        assert_eq!(team.last_round_net, summary.net);
        assert_eq!(team.cash_balance, 554_500.0);
    }

    #[test]
    fn amortization_pays_down_debt_from_cash_surplus() {
        let mut team = placeholder_team_from_db(
            "T001".to_string(),
            "Equipe Endividada".to_string(),
            "gt3".to_string(),
            "2026-01-01".to_string(),
        );
        team.cash_balance = 4_000_000.0;
        team.debt_balance = 5_000_000.0;

        // Rodada equilibrada (net = 0): a amortização vem só do caixa acumulado.
        apply_round_cashflow(
            &mut team,
            TeamRoundFinanceContext {
                linhas: Default::default(),
                sponsorship_income: 100_000.0,
                gate_income: 0.0,
                result_bonus: 0.0,
                partial_prize_income: 0.0,
                aid_income: 0.0,
                salary_expense: 100_000.0,
                event_operations_cost: 0.0,
                structural_maintenance_cost: 0.0,
                technical_investment_cost: 0.0,
                debt_service_cost: 0.0,
                ..Default::default()
            },
        );

        assert!(
            team.debt_balance < 5_000_000.0,
            "dívida deve cair (era 5M, ficou {})",
            team.debt_balance
        );
        // Caixa + dívida quitada deve ser conservado: o pagamento sai do caixa.
        assert!(team.cash_balance < 4_000_000.0);
        let paid = 5_000_000.0 - team.debt_balance;
        assert!((team.cash_balance - (4_000_000.0 - paid)).abs() < 1.0);
    }

    #[test]
    fn amortization_skipped_when_cash_below_reserve() {
        let mut team = placeholder_team_from_db(
            "T002".to_string(),
            "Equipe Quebrada".to_string(),
            "gt3".to_string(),
            "2026-01-01".to_string(),
        );
        team.cash_balance = -50_000.0;
        team.debt_balance = 6_000_000.0;

        apply_round_cashflow(
            &mut team,
            TeamRoundFinanceContext {
                linhas: Default::default(),
                sponsorship_income: 50_000.0,
                gate_income: 0.0,
                result_bonus: 0.0,
                partial_prize_income: 0.0,
                aid_income: 0.0,
                salary_expense: 50_000.0,
                event_operations_cost: 0.0,
                structural_maintenance_cost: 0.0,
                technical_investment_cost: 0.0,
                debt_service_cost: 0.0,
                ..Default::default()
            },
        );

        assert_eq!(
            team.debt_balance, 6_000_000.0,
            "sem caixa acima da reserva, nada de dívida é pago"
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
