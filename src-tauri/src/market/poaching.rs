//! Poaching / quebra de contrato na janela de mercado (Fase 2b do estrelato).
//! **Lógica PURA:** a multa de rescisão, o "vale a pena arrancar?" do assediante
//! e o leilão de salário em que o time atual briga pra segurar o piloto.
//! O dinheiro time→time e o passe na janela ficam no `pipeline` (tocam o banco).
//!
//! Regra travada com o user: a quebra acontece na JANELA (off-season), não no meio
//! da temporada — um piloto SOB contrato pode ser arrancado por um time disposto a
//! pagar a multa. Gatilho = FAMA + MÉRITO.

use crate::fame::fame_commercial_units;

/// Multiplicador de pedigree da multa: mistura MÉRITO (skill) e FAMA. Um astro custa
/// muito mais pra arrancar que um medíocre anônimo. Faixa ~[0.6 .. 1.8].
pub fn poach_pedigree(skill: f64, fama: f64) -> f64 {
    let skill_norm = ((skill.clamp(0.0, 100.0) - 40.0) / 60.0).clamp(0.0, 1.0);
    let fame_norm = fama.clamp(0.0, 100.0) / 100.0;
    0.6 + 0.7 * skill_norm + 0.5 * fame_norm
}

/// Multa de rescisão pra arrancar um piloto SOB contrato: `salário × anos_restantes
/// × pedigree`. `years_remaining` clampa em ≥1 (só se poacha quem tem ao menos 1 ano
/// pela frente; quem expira vira agente livre naturalmente). Nunca negativa.
pub fn buyout_fee(salary_anual: f64, years_remaining: i32, skill: f64, fama: f64) -> f64 {
    let years = years_remaining.max(1) as f64;
    (salary_anual.max(0.0) * years * poach_pedigree(skill, fama)).round()
}

/// Valor de um piloto para um assediante: mérito (skill) + apelo comercial da fama
/// ponderado pela necessidade do time (o MESMO termo da escada). Base pra decidir se
/// o alvo é upgrade que justifica a multa.
pub fn poach_target_value(skill: f64, fama: f64, need_factor: f64) -> f64 {
    skill + fame_commercial_units(fama) * need_factor
}

/// Teto de anos que a multa de PROMOÇÃO cobra, mesmo em contrato mais longo.
pub const ANOS_MAXIMOS_DA_MULTA_DE_PROMOCAO: i32 = 2;

/// Multa para levar um piloto SOB contrato numa PROMOÇÃO, que é outra coisa do assédio
/// e por isso tem régua própria: aqui a multa paga o CONTRATO, não o piloto.
///
/// A diferença importa porque as duas situações são opostas. No assédio é uma equipe
/// grande arrancando uma estrela, e cobrar pelo talento (`poach_pedigree`, faixa 0,6 a
/// 1,8) é exatamente o freio que se quer. Na promoção é uma equipe pobre subindo o
/// campeão da categoria de baixo, e cobrar pelo talento inverte o mecanismo: quanto
/// melhor o piloto, menos ele consegue subir.
///
/// Medido em 17/08/2026, com esta multa usando a régua do assédio: 48% a 65% das
/// promoções eram recusadas por caixa, no endurance 93%, e quem era recusado tinha 12 a
/// 15 pontos de skill A MAIS que quem subia. Um terço dos recusados nunca subia, e 2,5 a
/// 3,4 campeões por mundo de 15 temporadas ficavam presos. A régua nova tem três
/// diferenças, todas na direção de deixar o campeão passar:
///
/// 1. Sem o multiplicador de pedigree. A multa deixa de crescer com skill e fama.
/// 2. No máximo [`ANOS_MAXIMOS_DA_MULTA_DE_PROMOCAO`] anos, mesmo em contrato de três.
/// 3. `None` na última temporada de contrato: quem está de saída sobe de graça.
///
/// O teto de caixa continua sendo o [`can_afford_buyout`], compartilhado com o assédio.
pub fn buyout_fee_promocao(salary_anual: f64, years_remaining: i32) -> Option<f64> {
    if years_remaining < 2 {
        return None;
    }
    let years = years_remaining.min(ANOS_MAXIMOS_DA_MULTA_DE_PROMOCAO) as f64;
    Some((salary_anual.max(0.0) * years).round())
}

/// Margem mínima de UPGRADE (em pontos de valor) pra o assediante topar a dor de
/// cabeça de arrancar um contratado — evita troca-troca sem ganho real.
pub const POACH_UPGRADE_MARGIN: f64 = 8.0;

/// Fração MÁXIMA do caixa que um time topa queimar numa multa (não esvazia o cofre).
pub const POACH_CASH_FRACTION: f64 = 0.6;

/// O alvo é um upgrade claro sobre o piloto que ele substituiria?
pub fn is_clear_upgrade(target_value: f64, incumbent_value: f64) -> bool {
    target_value >= incumbent_value + POACH_UPGRADE_MARGIN
}

/// O time consegue pagar a multa sem torrar o caixa?
pub fn can_afford_buyout(team_cash: f64, buyout: f64) -> bool {
    buyout > 0.0 && buyout <= team_cash * POACH_CASH_FRACTION
}

// ---------------------------------------------------------------------------
// Fase 2b.2 — LEILÃO DE SALÁRIO (o time atual briga pra segurar o piloto).
// ---------------------------------------------------------------------------

/// Peso da qualidade do time e do vínculo no apelo de uma proposta, medidos na
/// MESMA moeda do salário: **100 pontos de apelo = dobrar o salário atual**.
/// Assim um time melhor "vale um desconto" e um vínculo alto exige um puxão bem
/// maior de dinheiro pra arrancar o piloto.
pub const APPEAL_QUALITY_WEIGHT: f64 = 0.35;
pub const APPEAL_BOND_WEIGHT: f64 = 0.50;

/// Folga mínima de apelo pra um lance "passar" o rival — sem isso o leilão
/// empata em cima do decimal e nunca converge.
pub const AUCTION_APPEAL_EDGE: f64 = 2.0;

/// Teto de rodadas: guarda-corpo puro. Com o salto de [`AUCTION_STEP`] os tetos
/// de salário sempre decidem antes disto — o cap NUNCA deve ser quem escolhe o
/// vencedor (senão a paridade dos lances viraria desempate).
pub const AUCTION_MAX_ROUNDS: usize = 24;

/// Agressividade do lance: ninguém cobre o rival no centavo — sobe com folga
/// (limitado ao próprio teto, e aí é o lance final "all-in"). É o que faz o
/// leilão convergir em poucas rodadas em vez de subir de migalha em migalha.
pub const AUCTION_STEP: f64 = 1.15;

/// Fração do caixa que um time topa comprometer em salário anual.
pub const WAGE_CASH_FRACTION: f64 = 0.35;

/// O quanto uma proposta atrai o piloto: dinheiro (relativo ao que ele ganha
/// hoje) + qualidade do time + vínculo com QUEM está propondo.
pub fn offer_appeal(salary: f64, reference_salary: f64, team_quality: f64, bond: f64) -> f64 {
    let reference = reference_salary.max(1.0);
    100.0 * (salary.max(0.0) / reference)
        + team_quality.max(0.0) * APPEAL_QUALITY_WEIGHT
        + bond.clamp(0.0, 100.0) * APPEAL_BOND_WEIGHT
}

/// Inverso de [`offer_appeal`]: o salário que este lado precisa oferecer pra
/// chegar em `target_appeal`. Nunca abaixo do salário atual (ninguém "segura"
/// um piloto cortando o salário dele).
pub fn salary_for_appeal(
    target_appeal: f64,
    reference_salary: f64,
    team_quality: f64,
    bond: f64,
) -> f64 {
    let reference = reference_salary.max(1.0);
    let non_money =
        team_quality.max(0.0) * APPEAL_QUALITY_WEIGHT + bond.clamp(0.0, 100.0) * APPEAL_BOND_WEIGHT;
    (((target_appeal - non_money) / 100.0) * reference).max(reference)
}

/// Teto de salário de um time por um piloto: escala com o quanto ele vale pro
/// time ([`poach_target_value`]) e é limitado pelo caixa.
pub fn salary_ceiling(reference_salary: f64, driver_value: f64, team_cash: f64) -> f64 {
    let value_norm = ((driver_value - 50.0) / 80.0).clamp(0.0, 1.0);
    let want = reference_salary.max(0.0) * (1.0 + value_norm);
    want.min((team_cash.max(0.0)) * WAGE_CASH_FRACTION)
}

/// Um lado do leilão.
#[derive(Debug, Clone)]
pub struct AuctionSide {
    pub team_id: String,
    pub team_quality: f64,
    /// Vínculo do piloto com ESTE time (0–100).
    pub bond: f64,
    /// Salário máximo que este time topa pagar.
    pub ceiling: f64,
}

/// Um lance do leilão (na ordem em que aconteceu — vira o histórico da tela no 2b.3).
#[derive(Debug, Clone)]
pub struct AuctionBid {
    pub team_id: String,
    pub salary: f64,
}

#[derive(Debug, Clone)]
pub struct AuctionResult {
    /// `true` = o assediante levou o piloto; `false` = o time atual segurou.
    pub poacher_wins: bool,
    /// Salário do contrato vencedor (pode ser o atual, se o assediante nem
    /// conseguiu cobrir o status quo).
    pub salary: f64,
    pub bids: Vec<AuctionBid>,
}

/// Leilão de salário ao vivo. O **status quo é o lance de abertura**: o piloto
/// já está no time atual, ganhando `reference_salary`, com o vínculo dele. O
/// assediante age primeiro e precisa superar isso; daí em diante cada lado cobre
/// o outro até alguém estourar o próprio teto. Quem lidera quando o rival desiste
/// leva o piloto.
pub fn resolve_salary_auction(
    reference_salary: f64,
    poacher: &AuctionSide,
    holder: &AuctionSide,
) -> AuctionResult {
    let reference = reference_salary.max(1.0);
    let mut bids = vec![AuctionBid {
        team_id: holder.team_id.clone(),
        salary: reference,
    }];
    let mut leader_is_poacher = false;
    let mut leader_salary = reference;
    let mut leader_appeal = offer_appeal(reference, reference, holder.team_quality, holder.bond);

    for _ in 0..AUCTION_MAX_ROUNDS {
        let challenger = if leader_is_poacher { holder } else { poacher };
        let needed = salary_for_appeal(
            leader_appeal + AUCTION_APPEAL_EDGE,
            reference,
            challenger.team_quality,
            challenger.bond,
        )
        .round();
        if needed > challenger.ceiling {
            break; // desistiu: quem lidera leva
        }
        // Cobre com folga; se a folga passa do teto, vai de lance final (all-in).
        let bid = (needed * AUCTION_STEP).round().min(challenger.ceiling);
        leader_is_poacher = !leader_is_poacher;
        leader_salary = bid;
        leader_appeal = offer_appeal(bid, reference, challenger.team_quality, challenger.bond);
        bids.push(AuctionBid {
            team_id: challenger.team_id.clone(),
            salary: bid,
        });
    }

    AuctionResult {
        poacher_wins: leader_is_poacher,
        salary: leader_salary,
        bids,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_multa_de_promocao_nao_cobra_o_ultimo_ano_de_contrato() {
        assert_eq!(buyout_fee_promocao(100_000.0, 1), None);
        assert_eq!(buyout_fee_promocao(100_000.0, 0), None);
        assert_eq!(buyout_fee_promocao(100_000.0, 2), Some(200_000.0));
    }

    #[test]
    fn a_multa_de_promocao_para_de_crescer_em_dois_anos() {
        assert_eq!(buyout_fee_promocao(100_000.0, 2), Some(200_000.0));
        assert_eq!(buyout_fee_promocao(100_000.0, 3), Some(200_000.0));
        assert_eq!(buyout_fee_promocao(100_000.0, 9), Some(200_000.0));
    }

    #[test]
    fn a_multa_de_promocao_ignora_talento_e_fama() {
        // O craque e o medíocre com o MESMO contrato pagam a mesma multa. É o que
        // distingue esta régua da do assédio, onde o pedigree multiplica por até 1,8.
        let craque = buyout_fee_promocao(100_000.0, 3);
        let mediocre = buyout_fee_promocao(100_000.0, 3);
        assert_eq!(craque, mediocre);
        assert!(buyout_fee(100_000.0, 3, 95.0, 90.0) > buyout_fee(100_000.0, 3, 45.0, 5.0));
    }

    #[test]
    fn a_multa_de_promocao_e_muito_menor_que_a_do_assedio_para_o_campeao() {
        // O caso que travava a escada: campeão de skill alta e mídia alta, três anos de
        // contrato. A régua do assédio cobrava 5,4 salários; a da promoção cobra 2.
        let assedio = buyout_fee(100_000.0, 3, 100.0, 100.0);
        let promocao = buyout_fee_promocao(100_000.0, 3).expect("tem multa");
        assert_eq!(assedio, 540_000.0);
        assert_eq!(promocao, 200_000.0);
        assert!(promocao < assedio / 2.0);
    }

    fn side(id: &str, quality: f64, bond: f64, ceiling: f64) -> AuctionSide {
        AuctionSide {
            team_id: id.to_string(),
            team_quality: quality,
            bond,
            ceiling,
        }
    }

    #[test]
    fn craque_famoso_custa_muito_mais_que_mediocre_anonimo() {
        let astro = buyout_fee(300_000.0, 2, 92.0, 90.0);
        let mediocre = buyout_fee(300_000.0, 2, 60.0, 15.0);
        assert!(astro > mediocre * 1.4, "astro={astro}, mediocre={mediocre}");
        // Ordem de grandeza esperada do exemplo do design (~$720k pro astro de 2 anos).
        assert!((600_000.0..=1_200_000.0).contains(&astro), "astro={astro}");
    }

    #[test]
    fn multa_escala_com_anos_restantes() {
        let um = buyout_fee(200_000.0, 1, 80.0, 60.0);
        let tres = buyout_fee(200_000.0, 3, 80.0, 60.0);
        // ~3× (folga de arredondamento — cada chamada faz `.round()`).
        assert!((tres - um * 3.0).abs() <= 3.0, "um={um}, tres={tres}");
    }

    #[test]
    fn anos_zero_ou_negativo_conta_como_um() {
        // Quem "expira" ainda tem multa de 1 ano se for arrancado antes de virar livre.
        assert_eq!(
            buyout_fee(100_000.0, 0, 70.0, 50.0),
            buyout_fee(100_000.0, 1, 70.0, 50.0)
        );
    }

    #[test]
    fn pedigree_limitado_e_monotonico() {
        assert!(poach_pedigree(40.0, 0.0) >= 0.6 - 1e-9);
        assert!(poach_pedigree(100.0, 100.0) <= 1.8 + 1e-9);
        assert!(poach_pedigree(92.0, 90.0) > poach_pedigree(60.0, 20.0));
    }

    #[test]
    fn upgrade_exige_margem() {
        // Igual não é upgrade; precisa passar da margem.
        assert!(!is_clear_upgrade(80.0, 80.0));
        assert!(!is_clear_upgrade(85.0, 80.0)); // +5 < margem 8
        assert!(is_clear_upgrade(90.0, 80.0)); // +10 ≥ margem
    }

    #[test]
    fn so_paga_multa_que_cabe_no_caixa() {
        // Caixa $1mi, fração 0.6 → topa até $600k.
        assert!(can_afford_buyout(1_000_000.0, 500_000.0));
        assert!(!can_afford_buyout(1_000_000.0, 700_000.0));
        // Multa nula/negativa nunca "cabe" (não há o que pagar).
        assert!(!can_afford_buyout(1_000_000.0, 0.0));
    }

    // --- Fase 2b.2: leilão ---

    #[test]
    fn time_rico_e_grande_arranca_piloto_de_time_pequeno() {
        // Assediante: melhor time, caixa gordo, sem vínculo. Atual: time fraco, pobre.
        let poacher = side("TRICO", 140.0, 0.0, 600_000.0);
        let holder = side("TPOBRE", 40.0, 10.0, 220_000.0);
        let r = resolve_salary_auction(200_000.0, &poacher, &holder);
        assert!(r.poacher_wins, "bids={:?}", r.bids);
        assert!(r.salary <= poacher.ceiling);
    }

    #[test]
    fn vinculo_de_casa_segura_o_piloto_contra_dinheiro() {
        // Mesmo porte de time: quem tem "Casa" (vínculo 95) segura — o vínculo vale
        // ~+48% de salário, e o assediante não tem caixa pra cobrir isso.
        let poacher = side("TX", 100.0, 0.0, 280_000.0);
        let holder = side("TCASA", 100.0, 95.0, 300_000.0);
        let r = resolve_salary_auction(200_000.0, &poacher, &holder);
        assert!(!r.poacher_wins, "bids={:?}", r.bids);
        // E nem precisou dar aumento: o assediante não cobriu o status quo.
        assert_eq!(r.salary, 200_000.0);
        assert_eq!(r.bids.len(), 1);
    }

    #[test]
    fn vinculo_alto_e_superavel_por_dinheiro_de_sobra() {
        // O mesmo vínculo "Casa" cai se o assediante tiver caixa pra bancar o puxão.
        let poacher = side("TX", 100.0, 0.0, 1_000_000.0);
        let holder = side("TCASA", 100.0, 95.0, 260_000.0);
        let r = resolve_salary_auction(200_000.0, &poacher, &holder);
        assert!(r.poacher_wins, "bids={:?}", r.bids);
        // Custou caro: bem acima do salário atual.
        assert!(r.salary > 250_000.0, "salary={}", r.salary);
    }

    #[test]
    fn reter_custa_caro_o_salario_sobe_no_contrato() {
        // Atual segura, mas só cobrindo — o salário final fica acima do de partida.
        let poacher = side("TX", 100.0, 0.0, 300_000.0);
        let holder = side("TATUAL", 100.0, 20.0, 900_000.0);
        let r = resolve_salary_auction(200_000.0, &poacher, &holder);
        assert!(!r.poacher_wins, "bids={:?}", r.bids);
        assert!(r.salary > 200_000.0, "salary={}", r.salary);
        assert!(r.salary <= holder.ceiling);
        // Houve troca de lances de verdade (status quo + assediante + cobertura).
        assert!(r.bids.len() >= 3, "bids={:?}", r.bids);
    }

    #[test]
    fn leilao_sempre_termina_e_respeita_os_tetos() {
        // Dois gigantes iguais e riquíssimos: converge sem estourar o teto de rodadas.
        let poacher = side("TA", 150.0, 40.0, 5_000_000.0);
        let holder = side("TB", 150.0, 40.0, 5_000_000.0);
        let r = resolve_salary_auction(200_000.0, &poacher, &holder);
        assert!(r.bids.len() <= AUCTION_MAX_ROUNDS + 1);
        assert!(r.salary <= 5_000_000.0);
        // Cada lance sobe de verdade.
        for pair in r.bids.windows(2) {
            assert!(pair[1].salary > pair[0].salary, "bids={:?}", r.bids);
        }
    }

    #[test]
    fn teto_de_salario_respeita_o_caixa_e_o_valor_do_piloto() {
        // Astro (valor alto) num time rico: topa quase dobrar o salário.
        let astro = salary_ceiling(200_000.0, 120.0, 10_000_000.0);
        assert!(astro > 360_000.0, "astro={astro}");
        // O mesmo astro num time sem caixa: o cofre manda.
        let duro = salary_ceiling(200_000.0, 120.0, 500_000.0);
        assert_eq!(duro, 500_000.0 * WAGE_CASH_FRACTION);
        // Piloto mediano: o time não estica o salário mesmo tendo caixa.
        let mediano = salary_ceiling(200_000.0, 55.0, 10_000_000.0);
        assert!(mediano < astro, "mediano={mediano}, astro={astro}");
    }
}
