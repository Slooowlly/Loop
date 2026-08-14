use crate::models::team::Team;

#[derive(Debug, Clone, PartialEq)]
pub struct FinanceEventOutcome {
    pub kind: String,
    pub cash_delta: f64,
    pub debt_delta: f64,
}

pub fn debt_service(debt_balance: f64, round_interest_rate: f64) -> f64 {
    if debt_balance <= 0.0 {
        return 0.0;
    }

    debt_balance * round_interest_rate.max(0.0)
}

pub fn debt_interest_rate_for_state(state: &str) -> f64 {
    match state {
        "elite" | "healthy" => 0.0075,
        "stable" => 0.0125,
        "pressured" => 0.020,
        "crisis" => 0.0325,
        "collapse" => 0.050,
        _ => 0.015,
    }
}

pub fn debt_service_for_state(debt_balance: f64, state: &str) -> f64 {
    debt_service(debt_balance, debt_interest_rate_for_state(state))
}

// ── Socorro de emergência ─────────────────────────────────────────────────────────────────
//
// # O que estava errado, medido
//
// A versão anterior decidia tudo em reais absolutos: gatilho em `caixa < −75 mil` **OU**
// `dívida ≥ 750 mil`, e principal de uma tabela por categoria (150 mil na Rookie, 800 mil no
// Endurance) cuja escada não era a escada do custo operacional. O harness de economia mostrou
// um mecanismo que trabalhava contra o próprio objetivo:
//
// - o socorro injeta caixa E aumenta a dívida em `1,18 × principal`, então a métrica de saúde
//   do jogo (`caixa − dívida`, em meses de operação) PIORA com o próprio socorro;
// - a dívida era uma das duas condições que ABRIAM o gatilho, então o socorro anterior
//   qualificava a equipe para o próximo — o `OU` fechava o laço;
// - ~98% dos tomadores voltavam a tomar, com dezenas de socorros por tomador;
// - o braço com socorro terminava com MAIS colapso e MAIS vendas que o braço sem socorro.
//
// # A política nova, e por que cada peça
//
// Tudo em MESES DE OPERAÇÃO da divisão (`finance::state::custo_operacional_mensal`), que é a
// unidade em que o jogo inteiro mede fôlego desde a troca da âncora de estoque. É o que faz a
// mesma regra valer na Rookie e no Endurance LMP2 sem tabela, e é o que torna a classe do
// Endurance visível: GT4 e LMP2 param de dividir a mesma linha de 800 mil.
//
// 1. **Necessidade** ([`SOCORRO_GATE_CAIXA_MESES`]): só entra quem está de fato sem caixa.
// 2. **Teto** ([`SOCORRO_TETO_DIVIDA_MESES`]): a dívida agora BARRA o socorro em vez de
//    liberá-lo. É a inversão que quebra o laço — quem já deve demais não recebe mais crédito,
//    que é o que um credor de verdade faz. Como o socorro cria `2 × 1,18 = 2,36` meses de
//    dívida, dois deles levam a equipe a 4,72 meses, acima do teto: a dívida criada tem fim
//    por construção, sem depender do contador.
// 3. **Limite por temporada** ([`SOCORROS_MAX_POR_TEMPORADA`]): a trava explícita, para o
//    mecanismo não depender da aritmética acima continuar valendo se algum coeficiente mudar.

/// Caixa (em meses de operação negativos) abaixo do qual a equipe precisa de socorro.
///
/// Dois meses é o piso da faixa declarada de fôlego do mundo (`economia::temporada::
/// faixa_de_caixa` vai de 1 a 11): abaixo disso a equipe não está apertada, está sem operação.
pub const SOCORRO_GATE_CAIXA_MESES: f64 = 2.0;

/// Dívida (em meses de operação) a partir da qual NÃO há mais socorro.
///
/// É TETO, nunca gatilho. Quatro meses são exatamente dois socorros cheios de
/// [`SOCORRO_PRINCIPAL_MESES`] — o crédito que o mundo estende a uma equipe em colapso, e nada
/// além disso. O teto e [`SOCORROS_MAX_POR_TEMPORADA`] param no mesmo ponto de propósito: um é
/// aritmético e o outro é explícito, e nenhum dos dois depende do outro continuar valendo.
pub const SOCORRO_TETO_DIVIDA_MESES: f64 = 4.0;

/// Principal do socorro, em meses de operação da divisão.
///
/// Dois meses é o mesmo aporte com que a nova diretoria de uma equipe VENDIDA volta a operar
/// (`finance::rescue::SALE_CAIXA_MESES`). Os dois números respondem à mesma pergunta — quanto
/// custa manter uma equipe de pé pelo mínimo — e discordar deles seria dizer que socorrer paga
/// melhor que quebrar.
pub const SOCORRO_PRINCIPAL_MESES: f64 = 2.0;

/// Quantos socorros uma equipe pode receber na mesma temporada.
pub const SOCORROS_MAX_POR_TEMPORADA: i32 = 2;

/// Quanto o socorro soma à dívida, por unidade de principal, NO ATO.
///
/// **Continua capitalizado de propósito.** O harness comparou este modelo com a alternativa
/// amortizada (o custo vira despesa de caixa rateada nas rodadas em vez de entrar na dívida no
/// ato) e a amortizada foi pior em todas as colunas que importam: mais colapso (21,33 contra
/// 20,97), mais dívida criada (27,6 contra 26,1 meses), mais juro pago (97,7 contra 91,0) e
/// mais socorros por tomador (2,67 contra 2,52). Ela adia o reconhecimento do passivo, e com a
/// dívida virando TETO, subestimá-la é subestimar o freio.
///
/// # A taxa de originação de 18% saiu, e o número é 1,00
///
/// Ela era `1,18`: o socorro entregava 2 meses de caixa e criava 2,36 meses de dívida. Como a
/// saúde do mundo é `caixa − dívida` em meses de operação, o socorro **piorava o indicador no
/// instante em que era concedido** — a primeira coisa que a medição de B50 apontou.
///
/// Medido no harness de economia (20 temporadas × réplicas, 7 categorias), colapso médio do
/// mundo, com todos os outros parâmetros iguais:
///
/// | taxa | colapso% | dívida criada (meses) | socorros por tomador |
/// |---|---|---|---|
/// | sem socorro (piso) | 20,92 | 0 | 0 |
/// | 1,18 | 21,49 | 26,4 | 2,17 |
/// | 1,08 | 21,22 | 27,7 | 2,60 |
/// | **1,00** | **20,97** | 26,1 | 2,52 |
/// | absoluta antiga | 24,23 | 1046,3 | 45,42 |
///
/// Só em 1,00 o braço com socorro encosta no piso do braço SEM socorro (20,97 contra 20,92, um
/// vigésimo de ponto percentual). Nenhum valor zera essa diferença, e isso é estrutural: a
/// equipe socorrida continua operando em colapso em vez de ser vendida, e a venda é justamente
/// o que tira uma equipe da conta. O que a taxa controlava era o resto, e o resto some aqui.
///
/// **Socorro continua não sendo de graça**, e o custo não é a taxa: é o JURO. A dívida em
/// banda de colapso paga 5% por rodada (`debt_interest_rate_for_state`), e a medição mostra
/// que o juro pago é ~3,5× a dívida criada ao longo da simulação. A taxa de originação era
/// punição redundante, e era a única parcela que fazia o remédio piorar o diagnóstico no ato.
pub const SOCORRO_TAXA: f64 = 1.00;

/// Quantos socorros esta equipe já tomou na temporada `temporada`.
///
/// O contador é lido como zero quando ele se refere a outra temporada: é assim que o limite se
/// reinicia na virada do ano, sem um passo de transição que alguém possa esquecer de chamar.
pub fn socorros_ja_tomados(team: &Team, temporada: i32) -> i32 {
    if team.socorros_temporada_ref == temporada {
        team.socorros_na_temporada.max(0)
    } else {
        0
    }
}

/// O principal que esta equipe pode tomar nesta temporada, ou `None` se ela não é elegível.
///
/// Os quatro portões, na ordem: estado de colapso, necessidade de caixa, teto de dívida e
/// limite por temporada. Ver o bloco de comentário acima para o porquê de cada um.
pub fn emergency_loan_amount_na_temporada(team: &Team, temporada: i32) -> Option<f64> {
    if team.financial_state != "collapse" {
        return None;
    }

    let mensal =
        crate::finance::state::custo_operacional_mensal(&team.categoria, team.classe.as_deref());
    if mensal <= 0.0 {
        return None;
    }

    // Necessidade.
    if team.cash_balance > -SOCORRO_GATE_CAIXA_MESES * mensal {
        return None;
    }
    // Teto de dívida. É `>=` para o teto ser alcançável: uma equipe exatamente no teto já
    // esgotou o crédito.
    if team.debt_balance >= SOCORRO_TETO_DIVIDA_MESES * mensal {
        return None;
    }
    // Limite por temporada.
    if socorros_ja_tomados(team, temporada) >= SOCORROS_MAX_POR_TEMPORADA {
        return None;
    }

    // A modulação por reputação é a de sempre: equipe mais respeitada consegue crédito um pouco
    // melhor. O que mudou é a BASE que ela modula.
    Some(SOCORRO_PRINCIPAL_MESES * mensal * (0.85 + team.reputacao.clamp(0.0, 100.0) / 500.0))
}

pub fn technical_breakthrough_chance(team: &Team) -> f64 {
    if team.engineering < 70.0 {
        return 0.0;
    }

    let pressure_bonus = match team.financial_state.as_str() {
        "pressured" => 0.015,
        "crisis" => 0.025,
        "collapse" => 0.01,
        _ => 0.005,
    };
    let engineering_bonus = ((team.engineering - 70.0) / 30.0).clamp(0.0, 1.0) * 0.05;

    (pressure_bonus + engineering_bonus).clamp(0.0, 0.10)
}

/// Total do paraquedas de rebaixamento, em MESES de operação da divisão de DESTINO.
///
/// # Por que a tabela absoluta saiu
///
/// `category_parachute_payment_base` decidia tudo em reais e a escada dela não era a escada
/// do custo operacional. Medido (B47), o mesmo paraquedas comprava fôlegos incomparáveis:
/// **6,78 meses na Rookie contra 1,94 mês no Endurance LMP2**. A equipe que cai do degrau mais
/// caro, que é onde cair dói mais, recebia o menor alívio relativo do jogo.
///
/// Três meses é a leitura direta do que o paraquedas existe para fazer: bancar UM trimestre de
/// operação na divisão nova enquanto a equipe reencontra patrocínio e folha compatíveis com o
/// degrau em que caiu. Fica acima do piso de `pressionada` (3 meses em
/// [`crate::finance::state::FaixasDeMeses`]) e bem abaixo do caixa da equipe mediana (6 meses):
/// é socorro de aterrissagem, não um prêmio por ser rebaixado.
pub const PARAQUEDAS_MESES: f64 = 3.0;

/// O total do paraquedas de uma equipe recém-rebaixada.
///
/// Lê a divisão de DESTINO por construção: `promotion::pipeline` troca `categoria` e `classe`
/// (em `apply_team_category_change`) ANTES de aplicar os deltas do movimento, então a equipe
/// que chega aqui já é a equipe da divisão nova. Isso é o que torna a leitura class-aware sem
/// nenhum parâmetro extra — e é o que separa Endurance GT4 de Endurance LMP2, que a tabela
/// absoluta tratava como a mesma linha de 700 mil.
///
/// A modulação por reputação saiu junto com a tabela. Ela existia para separar equipes dentro
/// de um degrau; o que separava de verdade era o degrau, e mantê-la faria o total deixar de ser
/// divisível pelas rodadas da temporada — que é justamente a garantia de o saldo acabar dentro
/// de uma temporada (ver [`parcela_de_paraquedas`]).
pub fn parachute_payment_for_relegation(team: &Team) -> f64 {
    total_de_paraquedas(&team.categoria, team.classe.as_deref())
}

/// [`parachute_payment_for_relegation`] pela divisão, sem precisar de uma `Team`.
pub fn total_de_paraquedas(categoria: &str, classe: Option<&str>) -> f64 {
    PARAQUEDAS_MESES * crate::finance::state::custo_operacional_mensal(categoria, classe)
}

/// A PARCELA que a etapa paga: o total dividido pelas rodadas da temporada de destino.
///
/// A parcela fixa de 25 mil por rodada tinha o mesmo defeito da tabela, elevado: ela era igual
/// em toda a escada, então na Rookie liquidava o saldo em poucas etapas e no Endurance o
/// arrastava por **até ~4,6 temporadas**. Paraquedas que sobrevive a quatro temporadas deixou
/// de ser paraquedas: vira renda permanente de quem caiu.
///
/// Dividir o total pelas rodadas faz o saldo acabar exatamente na última etapa da temporada de
/// destino, em qualquer divisão — que é a política pedida, e é a mesma unidade relativa em
/// todas as classes.
pub fn parcela_de_paraquedas(categoria: &str, classe: Option<&str>, rodadas: f64) -> f64 {
    total_de_paraquedas(categoria, classe) / rodadas.max(1.0)
}

/// Aplica o socorro, se houver, e REGISTRA que ele aconteceu.
///
/// O registro é o que faz o limite por temporada existir; sem ele os portões voltariam a ser
/// só a foto do balanço, que é o defeito original.
pub fn apply_crisis_event_if_needed(
    team: &mut Team,
    temporada: i32,
) -> Option<FinanceEventOutcome> {
    let loan_amount = emergency_loan_amount_na_temporada(team, temporada)?;
    let debt_delta = loan_amount * SOCORRO_TAXA;

    team.cash_balance += loan_amount;
    team.debt_balance += debt_delta;

    let ja = socorros_ja_tomados(team, temporada);
    team.socorros_na_temporada = ja + 1;
    team.socorros_temporada_ref = temporada;

    Some(FinanceEventOutcome {
        kind: "emergency_loan".to_string(),
        cash_delta: loan_amount,
        debt_delta,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::team::placeholder_team_from_db;

    fn sample_team(
        state: &str,
        cash: f64,
        debt: f64,
        engineering: f64,
    ) -> crate::models::team::Team {
        let mut team = placeholder_team_from_db(
            "T001".to_string(),
            "Equipe em Crise".to_string(),
            "gt4".to_string(),
            "2026-01-01".to_string(),
        );
        team.financial_state = state.to_string();
        team.cash_balance = cash;
        team.debt_balance = debt;
        team.engineering = engineering;
        team.reputacao = 55.0;
        team
    }

    #[test]
    fn debt_service_is_positive_when_team_owes_money() {
        assert!(debt_service(100_000.0, 0.015) > 0.0);
    }

    #[test]
    fn debt_service_is_harsher_for_collapse_than_healthy_state() {
        let debt = 1_000_000.0;

        let healthy = debt_service_for_state(debt, "healthy");
        let collapse = debt_service_for_state(debt, "collapse");

        assert!(collapse > healthy);
    }

    #[test]
    fn unknown_debt_service_state_uses_safe_default() {
        assert_eq!(
            debt_service_for_state(1_000_000.0, "mystery"),
            debt_service(1_000_000.0, 0.015)
        );
    }

    #[test]
    fn state_based_debt_service_stays_zero_without_debt() {
        assert_eq!(debt_service_for_state(0.0, "collapse"), 0.0);
    }

    /// As divisões que a política precisa atravessar sem tabela. As três do Endurance estão
    /// aqui porque é onde uma escala cega à classe custa mais: GT4 e LMP2 correm o mesmo
    /// campeonato e não custam o mesmo.
    const DIVISOES: &[(&str, Option<&str>)] = &[
        ("mazda_rookie", None),
        ("bmw_m2", None),
        ("gt4", None),
        ("gt3", None),
        ("endurance", Some("gt4")),
        ("endurance", Some("gt3")),
        ("endurance", Some("lmp2")),
    ];

    /// Equipe em colapso, com o caixa medido em MESES da própria divisão.
    fn equipe_afogada(categoria: &str, classe: Option<&str>, meses_de_caixa: f64) -> Team {
        let mut team = placeholder_team_from_db(
            "T001".to_string(),
            "Equipe em Crise".to_string(),
            categoria.to_string(),
            "2026-01-01".to_string(),
        );
        team.classe = classe.map(str::to_string);
        team.reputacao = 55.0;
        team.financial_state = "collapse".to_string();
        team.cash_balance = meses_de_caixa * mensal(categoria, classe);
        team.debt_balance = 0.0;
        team
    }

    fn mensal(categoria: &str, classe: Option<&str>) -> f64 {
        crate::finance::state::custo_operacional_mensal(categoria, classe)
    }

    /// **B50 — a unidade.** O principal é o MESMO número de meses de operação em toda a
    /// escada, incluindo as três classes do Endurance. É o teste que a tabela absoluta não
    /// tinha como passar: ela pagava 6,78 meses na Rookie e 1,94 no LMP2.
    #[test]
    fn o_socorro_vale_os_mesmos_meses_em_toda_a_escada() {
        for (categoria, classe) in DIVISOES {
            let team = equipe_afogada(categoria, *classe, -3.0);
            let principal = emergency_loan_amount_na_temporada(&team, 1)
                .unwrap_or_else(|| panic!("{categoria}/{classe:?} deveria ser elegível"));
            let meses = principal / mensal(categoria, *classe);
            // A modulação por reputação (0,85 + 55/500 = 0,96) é a mesma em toda a escada.
            let esperado = SOCORRO_PRINCIPAL_MESES * 0.96;
            assert!(
                (meses - esperado).abs() < 0.01,
                "{categoria}/{classe:?}: socorro vale {meses:.2} meses, esperado {esperado:.2}"
            );
        }
    }

    /// **B50 — as classes do Endurance não dividem a mesma linha.** A tabela antiga pagava
    /// 800 mil para as três; o LMP2 custa múltiplos do GT4 para operar.
    #[test]
    fn o_endurance_respeita_a_escala_de_cada_classe() {
        let gt4 =
            emergency_loan_amount_na_temporada(&equipe_afogada("endurance", Some("gt4"), -3.0), 1)
                .expect("gt4 elegível");
        let lmp2 =
            emergency_loan_amount_na_temporada(&equipe_afogada("endurance", Some("lmp2"), -3.0), 1)
                .expect("lmp2 elegível");
        assert!(
            lmp2 > gt4 * 2.0,
            "LMP2 ({lmp2:.0}) deveria tomar bem mais que GT4 ({gt4:.0})"
        );
    }

    /// **B50 — o gate de caixa é de necessidade.** Quem está em colapso mas com caixa acima
    /// do gate não recebe: o socorro é para quem parou de operar, não para quem está apertado.
    #[test]
    fn equipe_saudavel_e_equipe_apertada_nao_recebem() {
        // Saudável: nem entra pelo estado.
        let mut saudavel = equipe_afogada("gt3", None, -5.0);
        saudavel.financial_state = "stable".to_string();
        assert!(emergency_loan_amount_na_temporada(&saudavel, 1).is_none());

        // Em colapso, mas dentro do gate de caixa.
        let apertada = equipe_afogada("gt3", None, -(SOCORRO_GATE_CAIXA_MESES - 0.2));
        assert!(emergency_loan_amount_na_temporada(&apertada, 1).is_none());

        // Em colapso e fora do gate: recebe.
        let afogada = equipe_afogada("gt3", None, -(SOCORRO_GATE_CAIXA_MESES + 0.2));
        assert!(emergency_loan_amount_na_temporada(&afogada, 1).is_some());
    }

    /// **O contrato numérico de UM socorro**, nas quatro grandezas que ele move: caixa para
    /// cima pelo principal, dívida para cima por `principal × SOCORRO_TAXA`, contador em 1 e
    /// temporada de referência carimbada.
    ///
    /// Ele mora aqui, e não no fim de semana completo
    /// (`commands::race::tests::test_simulate_race_weekend_applies_crisis_finance_event`),
    /// porque lá o mundo é sorteado a cada execução e o resultado da rodada entra ANTES do
    /// gate. A janela em que o socorro sai é limitada dos dois lados — pelo gate de caixa em
    /// cima e pelo cheque especial de `cashflow.rs` em baixo, que empurra a dívida além do
    /// teto —, então rodada boa ou rodada péssima fecham o portão por motivos opostos. O
    /// teste de lá passou a asseverar o CONTRATO ("uma das três saídas aconteceu"), que não
    /// depende do sorteio, e o número exato ficou nesta bancada determinística.
    #[test]
    fn um_socorro_injeta_caixa_cria_divida_e_registra() {
        for (categoria, classe) in DIVISOES {
            let m = mensal(categoria, *classe);
            let mut team = equipe_afogada(categoria, *classe, -3.0);
            let caixa_antes = team.cash_balance;

            let principal = emergency_loan_amount_na_temporada(&team, 4)
                .expect("a equipe afogada deveria estar elegível");
            let evento = apply_crisis_event_if_needed(&mut team, 4)
                .expect("o socorro deveria sair para a equipe afogada");

            assert_eq!(evento.kind, "emergency_loan");
            assert!(
                (evento.cash_delta - principal).abs() < 1.0,
                "{categoria}/{classe:?}: caixa injetado {} contra principal {principal}",
                evento.cash_delta
            );
            assert!(
                (evento.debt_delta - principal * SOCORRO_TAXA).abs() < 1.0,
                "{categoria}/{classe:?}: dívida criada {} contra {}",
                evento.debt_delta,
                principal * SOCORRO_TAXA
            );
            assert!(
                (team.cash_balance - (caixa_antes + principal)).abs() < 1.0,
                "{categoria}/{classe:?}: o caixa da equipe não subiu o principal"
            );
            assert!(
                team.debt_balance > 0.0,
                "{categoria}/{classe:?}: o socorro tem que criar dívida"
            );
            assert_eq!(team.socorros_na_temporada, 1);
            assert_eq!(team.socorros_temporada_ref, 4);

            // O principal é sempre da ordem de SOCORRO_PRINCIPAL_MESES, modulado por
            // reputação: sem isso a asserção acima passaria com um principal de zero.
            assert!(
                principal > SOCORRO_PRINCIPAL_MESES * m * 0.8,
                "{categoria}/{classe:?}: principal {principal} pequeno demais"
            );
        }
    }

    /// **B50 — a dívida é TETO, não gatilho.** Este é o teste que fecha o laço medido: na
    /// política antiga a dívida acima de 750 mil ABRIA o socorro, e o socorro criava dívida.
    #[test]
    fn a_divida_barra_o_socorro_em_vez_de_liberar() {
        for (categoria, classe) in DIVISOES {
            let m = mensal(categoria, *classe);

            // Abaixo do teto, e com necessidade de caixa: passa.
            let mut abaixo = equipe_afogada(categoria, *classe, -3.0);
            abaixo.debt_balance = (SOCORRO_TETO_DIVIDA_MESES - 0.5) * m;
            assert!(
                emergency_loan_amount_na_temporada(&abaixo, 1).is_some(),
                "{categoria}/{classe:?}: dívida abaixo do teto deveria passar"
            );

            // Acima do teto: barra, por mais afundada que a equipe esteja.
            let mut acima = equipe_afogada(categoria, *classe, -20.0);
            acima.debt_balance = (SOCORRO_TETO_DIVIDA_MESES + 0.5) * m;
            assert!(
                emergency_loan_amount_na_temporada(&acima, 1).is_none(),
                "{categoria}/{classe:?}: dívida acima do teto deveria barrar"
            );
        }
    }

    /// **B50 — o limite por temporada.** Dois socorros e acabou; a terceira chamada no mesmo
    /// ano não devolve nada, mesmo com a equipe elegível por caixa e dívida.
    #[test]
    fn o_limite_de_socorros_por_temporada_vale() {
        let mut team = equipe_afogada("gt3", None, -3.0);
        for n in 1..=SOCORROS_MAX_POR_TEMPORADA {
            // O caixa é reposto à mão a cada rodada: o eixo aqui é o CONTADOR, e sem repor
            // o próprio socorro anterior tiraria a equipe do gate de necessidade.
            team.cash_balance = -3.0 * mensal("gt3", None);
            team.debt_balance = 0.0;
            assert!(
                apply_crisis_event_if_needed(&mut team, 7).is_some(),
                "socorro {n} da temporada deveria sair"
            );
            assert_eq!(team.socorros_na_temporada, n);
            assert_eq!(team.socorros_temporada_ref, 7);
        }
        team.cash_balance = -3.0 * mensal("gt3", None);
        team.debt_balance = 0.0;
        assert!(
            apply_crisis_event_if_needed(&mut team, 7).is_none(),
            "o socorro além do limite saiu"
        );

        // Virada de temporada: o orçamento volta, sem passo de transição nenhum.
        assert_eq!(socorros_ja_tomados(&team, 8), 0);
        assert!(
            apply_crisis_event_if_needed(&mut team, 8).is_some(),
            "a temporada nova deveria reabrir o socorro"
        );
        assert_eq!(team.socorros_na_temporada, 1);
        assert_eq!(team.socorros_temporada_ref, 8);
    }

    /// **B50 — o socorro anterior não qualifica o seguinte.** Sem repor caixa à mão, a
    /// sequência de socorros PARA sozinha: o principal tira a equipe do gate de necessidade e
    /// a dívida criada encosta no teto. A dívida gerada tem fim.
    #[test]
    fn a_divida_criada_pelo_socorro_nao_cresce_sem_limite() {
        for (categoria, classe) in DIVISOES {
            let m = mensal(categoria, *classe);
            let mut team = equipe_afogada(categoria, *classe, -3.0);
            let mut socorros = 0;
            // 60 rodadas: cinco temporadas de calendário cheio. Se a dívida fosse absorvente,
            // aqui apareceria.
            for rodada in 0..60 {
                if apply_crisis_event_if_needed(&mut team, 1 + rodada / 12).is_some() {
                    socorros += 1;
                }
            }
            assert!(
                team.debt_balance
                    <= (SOCORRO_TETO_DIVIDA_MESES + SOCORRO_PRINCIPAL_MESES * SOCORRO_TAXA) * m
                        + 1.0,
                "{categoria}/{classe:?}: dívida criada chegou a {:.2} meses",
                team.debt_balance / m
            );
            assert!(
                socorros <= SOCORROS_MAX_POR_TEMPORADA * 5,
                "{categoria}/{classe:?}: {socorros} socorros em 60 rodadas"
            );
        }
    }

    #[test]
    fn technical_breakthrough_requires_good_engineering() {
        let weak = sample_team("pressured", 50_000.0, 0.0, 35.0);
        let clever = sample_team("pressured", 50_000.0, 0.0, 82.0);

        assert_eq!(technical_breakthrough_chance(&weak), 0.0);
        assert!(technical_breakthrough_chance(&clever) > 0.0);
    }

    #[test]
    fn relegated_team_gets_parachute_payment() {
        let team = sample_team("stable", 200_000.0, 0.0, 55.0);

        let payment = parachute_payment_for_relegation(&team);

        assert!(payment > 0.0);
    }

    /// **B47 — o mesmo número relativo de meses entre classes.** A tabela absoluta comprava
    /// 6,78 meses na Rookie contra 1,94 no Endurance LMP2.
    #[test]
    fn o_paraquedas_vale_os_mesmos_meses_em_toda_a_escada() {
        for (categoria, classe) in DIVISOES {
            let mut team = sample_team("stable", 0.0, 0.0, 55.0);
            team.categoria = (*categoria).to_string();
            team.classe = classe.map(str::to_string);
            let meses = parachute_payment_for_relegation(&team) / mensal(categoria, *classe);
            assert!(
                (meses - PARAQUEDAS_MESES).abs() < 0.01,
                "{categoria}/{classe:?}: paraquedas vale {meses:.2} meses, esperado \
                 {PARAQUEDAS_MESES:.2}"
            );
        }
    }

    /// **B47 — o saldo acaba DENTRO de uma temporada.** A parcela fixa de 25 mil arrastava o
    /// saldo do Endurance por até ~4,6 temporadas; a parcela relativa liquida na última etapa,
    /// em qualquer divisão e para qualquer calendário.
    #[test]
    fn o_paraquedas_seca_na_ultima_etapa_da_temporada() {
        for (categoria, classe) in DIVISOES {
            for rodadas in [8.0, 10.0, 12.0, 16.0] {
                let mut saldo = total_de_paraquedas(categoria, *classe);
                let parcela = parcela_de_paraquedas(categoria, *classe, rodadas);
                let mut pagas = 0;
                while saldo > 1.0 {
                    saldo = (saldo - parcela.min(saldo)).max(0.0);
                    pagas += 1;
                    assert!(
                        pagas <= rodadas as i32,
                        "{categoria}/{classe:?}: o saldo passou de {rodadas} rodadas"
                    );
                }
                assert_eq!(
                    pagas, rodadas as i32,
                    "{categoria}/{classe:?}: secou em {pagas} de {rodadas} rodadas"
                );
                // E não paga nada depois de secar (a menos do resíduo de ponto flutuante).
                assert!(saldo.min(parcela) < 1.0);
            }
        }
    }

    /// **B47 — a leitura é da divisão de DESTINO.** Quem cai da GT3 para a GT4 recebe o
    /// paraquedas da GT4, que é onde o dinheiro vai ser gasto.
    #[test]
    fn o_paraquedas_le_a_divisao_de_destino() {
        let mut destino = sample_team("stable", 0.0, 0.0, 55.0);
        destino.categoria = "gt4".to_string();
        destino.classe = None;
        let mut origem = sample_team("stable", 0.0, 0.0, 55.0);
        origem.categoria = "gt3".to_string();
        origem.classe = None;

        assert!(
            parachute_payment_for_relegation(&destino) < parachute_payment_for_relegation(&origem),
            "o paraquedas da GT4 tem que ser menor que o da GT3"
        );
        assert!(
            (parachute_payment_for_relegation(&destino) - PARAQUEDAS_MESES * mensal("gt4", None))
                .abs()
                < 1.0
        );
    }

    #[test]
    fn applying_crisis_event_improves_cash_but_increases_debt() {
        let mut team = equipe_afogada("gt4", None, -3.0);
        let before_cash = team.cash_balance;
        let before_debt = team.debt_balance;

        let event = apply_crisis_event_if_needed(&mut team, 1).expect("event should be applied");

        assert_eq!(event.kind, "emergency_loan");
        assert!(team.cash_balance > before_cash);
        assert!(team.debt_balance > before_debt);
    }
}
