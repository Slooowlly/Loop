//! O RALO: o preço de crescer, e o que crescer compra.
//!
//! Substitui `finance::cashflow::apply_offseason_competitiveness_impact`, que hoje **regala
//! 2,76 pontos de estrutura por equipe por temporada sem debitar nada** — medido no harness
//! de economia. É uma das três fontes de dinheiro do nada da seção 2.5 do redesign, e a
//! razão declarada de o superávit não ter para onde ir (seção 2.6).
//!
//! ## O que a medição obrigou
//!
//! A seção 3.4 propunha duas formas de ralo, e a varredura do harness
//! (`varrer_ralo`) reprovou as duas:
//!
//! | forma | como condiciona | resultado medido |
//! |---|---|---|
//! | manter estrutura | proporcional ao PORTE | trava em 5/9 categorias no alvo; crise estoura antes da deriva ceder |
//! | melhorar estrutura, preço de tabela | proporcional aos PONTOS ganhos | só fecha cobrando 128× o operacional anual por 100 pontos, com 67% do mundo em crise |
//! | dreno do excedente | proporcional ao EXCEDENTE | 9/9 no alvo, crise em 12,2% |
//!
//! O motivo é uma correlação que não existe: **porte não prevê superávit.** Medido, o
//! superávit por temporada vai de −22,5% do custo operacional anual na GT3 (que é o segundo
//! maior porte da escada) a +90,5% na Toyota Rookie. Um custo proporcional ao porte cobra
//! caro de quem não tem e barato de quem tem.
//!
//! Daí a regra deste módulo: **a equipe investe o EXCEDENTE de caixa e recebe estrutura
//! proporcional ao que investiu, com retornos decrescentes.** Quem não tem excedente não
//! investe e não paga — a GT3 fica intocada por construção, e é isso que a medição pedia.
//!
//! ## As três decisões de modelagem, e por que
//!
//! 1. **O dinheiro sai inteiro; o RETORNO é que satura.** A equipe não para de gastar
//!    quando a estrutura encosta no teto — ela gasta em teste privado, em gente, em
//!    desenvolvimento que não vinga. Se o dinheiro fosse limitado ao que a estrutura
//!    consegue absorver, o ralo secaria justamente na equipe mais rica e o caixa voltaria a
//!    inflar. "Retorno decrescente" quer dizer literalmente isto: pagar mais por menos.
//! 2. **Estrutura DEPRECIA.** Sem depreciação o modelo vira catraca: todo mundo sobe até
//!    100 e o investimento perde sentido — e, pior, a equipe que chegou ao teto para de
//!    drenar. Com depreciação, ficar no topo custa todo ano, que é o freio que a seção 3.4
//!    procurava.
//! 3. **Nada aqui olha o banco.** Entrada e saída são structs puros, como o resto de
//!    `economia/`. O chamador aplica os deltas.

/// Os parâmetros do ralo. Os valores de [`Default`] são os que o harness mediu como
/// viáveis — ver `varrer_ralo` em `commands::race::tests::medicao_financeira`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParametrosDeDesenvolvimento {
    /// Quantos meses de operação a equipe segura antes de considerar que tem excedente.
    /// Abaixo disso ela não investe nada — é a reserva que a mantém viva.
    ///
    /// **Este número é o PISO do fôlego de toda equipe saudável do mundo**, e é por isso que
    /// ele acabou sendo o botão da calibração final em vez de qualquer coeficiente de
    /// receita: um ralo que se recusa a descer abaixo de N meses garante que ninguém termine
    /// 20 temporadas com menos de N. Com N = 12 e a faixa do critério 2 em 3–18, o alvo tinha
    /// só 6 meses de folga acima do piso, e cinco categorias saíam por cima (até 24,7).
    ///
    /// Desceu para 9 depois de duas varreduras (`varrer_a_reserva_do_ralo`). Os 3 meses a
    /// menos não são estética: é o único ponto medido em que o critério 7 fecha inteiro
    /// (crise 5,0% com colapso 2,3% < crise-pura 2,7%) sem abrir o critério 6.
    ///
    /// Consequência assumida: 9 meses fica um degrau ABAIXO da fronteira `saudavel` de
    /// [`crate::finance::state::FaixasDeMeses`] (12). A equipe de IA passa a repousar em
    /// "estável", não em "saudável" — ela investe em vez de entesourar, que é a leitura certa
    /// para equipe de corrida de cliente. As duas fronteiras deixaram de ser o mesmo número
    /// de propósito: uma é o que a equipe ESCOLHE guardar, a outra é como o estado é LIDO.
    pub meses_de_reserva: f64,
    /// Quanto do excedente a equipe converte em investimento por temporada. **0,40 é o
    /// número medido**: é a magnitude que põe as 9 categorias com deriva de caixa abaixo
    /// de 1,3× mantendo a crise em 12,2%.
    pub fracao_do_excedente: f64,
    /// Teto do investimento, em múltiplos do custo operacional anual. `None` = sem teto.
    ///
    /// **`None` é o padrão de propósito.** Com teto, o excedente acima do ponto em que o
    /// teto morde volta a acumular e a deriva de caixa reaparece nas categorias ricas —
    /// medido. O que limita o crescimento aqui é o retorno decrescente, não um limite de
    /// gasto.
    pub teto_do_investimento: Option<f64>,
    /// Máximo de pontos de estrutura que uma temporada de investimento compra, somando
    /// engenharia + instalações + confiabilidade. A assíntota da curva de retorno.
    pub pontos_maximos_por_temporada: f64,
    /// Investimento (em múltiplos do custo operacional anual) que compra 63% do máximo.
    /// É a meia-vida da saturação: quanto menor, mais rápido o retorno seca.
    pub escala_de_saturacao: f64,
    /// Fração da estrutura ATUAL que se perde por temporada sem investimento. Equipamento
    /// envelhece, gente vai embora, o galpão precisa de reforma.
    pub depreciacao_anual: f64,
}

impl Default for ParametrosDeDesenvolvimento {
    fn default() -> Self {
        Self {
            meses_de_reserva: 9.0,
            fracao_do_excedente: 0.40,
            teto_do_investimento: None,
            pontos_maximos_por_temporada: 6.0,
            escala_de_saturacao: 0.35,
            depreciacao_anual: 0.02,
        }
    }
}

/// Como a equipe reparte o que comprou. Soma 1,0.
///
/// Engenharia pesa mais porque é o que vira ritmo; confiabilidade pesa menos porque tem
/// retorno próprio no abandono. Não é um botão de balanceamento — é o desenho de para onde
/// o dinheiro vai.
const PESO_ENGENHARIA: f64 = 0.40;
const PESO_INSTALACOES: f64 = 0.35;
const PESO_CONFIABILIDADE: f64 = 0.25;

/// O estado da equipe no fim da temporada, na hora de decidir o investimento.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EntradaDeDesenvolvimento {
    pub caixa: f64,
    /// Dívida em aberto. Entra abatendo o caixa: equipe endividada não tem excedente,
    /// ela tem um passivo.
    pub divida: f64,
    /// O custo de operar uma temporada nesta categoria. É a unidade em que "meses de
    /// reserva" e "escala de saturação" fazem sentido.
    ///
    /// Entra como ARGUMENTO, e não vem de uma tabela daqui, porque quem sabe esse número é
    /// `economia::temporada`. Manter isto como entrada é o que deixa o módulo testável
    /// sozinho e o harness dirigir os dois modelos lado a lado.
    pub custo_operacional_anual: f64,
    /// Estrutura de hoje (0–100 cada).
    pub engenharia: f64,
    pub instalacoes: f64,
    pub confiabilidade: f64,
    /// Apetite da equipe (0–1), vindo do foco/estratégia. 1,0 investe a fração cheia;
    /// 0,0 é uma equipe que decidiu segurar caixa nesta temporada.
    pub apetite: f64,
}

impl EntradaDeDesenvolvimento {
    /// Excedente de caixa: o que sobra acima da reserva de operação, já abatida a dívida.
    /// Nunca negativo — quem está abaixo da reserva simplesmente não tem excedente.
    pub fn excedente(&self, p: &ParametrosDeDesenvolvimento) -> f64 {
        let reserva = self.custo_operacional_anual.max(0.0) * p.meses_de_reserva / 12.0;
        (self.caixa - self.divida - reserva).max(0.0)
    }
}

/// O que a equipe decidiu e o que isso comprou. Os deltas já vêm limitados: aplicá-los
/// nunca tira `engenharia`/`instalacoes`/`confiabilidade` da faixa 0–100.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PlanoDeDesenvolvimento {
    /// O excedente que existia antes de decidir.
    pub excedente: f64,
    /// O que sai do caixa. É o ralo.
    pub investimento: f64,
    /// Pontos de estrutura comprados, antes da depreciação.
    pub pontos_comprados: f64,
    /// Pontos perdidos para a depreciação.
    pub pontos_depreciados: f64,
    pub delta_engenharia: f64,
    pub delta_instalacoes: f64,
    pub delta_confiabilidade: f64,
}

impl PlanoDeDesenvolvimento {
    /// Movimento líquido de estrutura na temporada. Negativo quando a depreciação passa o
    /// que o investimento comprou — é o que acontece com quem não tem excedente.
    pub fn delta_liquido(&self) -> f64 {
        self.delta_engenharia + self.delta_instalacoes + self.delta_confiabilidade
    }
}

/// Quantos pontos de estrutura um investimento compra.
///
/// Saturante: `max × (1 − e^(−inv/escala))`. Dobrar o investimento nunca dobra o retorno, e
/// perto da assíntota o dinheiro praticamente não compra nada — que é o mecanismo pelo qual
/// o ralo drena a equipe rica sem transformar a escada esportiva em função do caixa.
pub fn pontos_por_investimento(
    investimento: f64,
    custo_operacional_anual: f64,
    p: &ParametrosDeDesenvolvimento,
) -> f64 {
    if investimento <= 0.0 || custo_operacional_anual <= 0.0 || p.escala_de_saturacao <= 0.0 {
        return 0.0;
    }
    let x = investimento / (custo_operacional_anual * p.escala_de_saturacao);
    p.pontos_maximos_por_temporada * (1.0 - (-x).exp())
}

/// A decisão de offseason de uma equipe: quanto investir e o que isso move.
///
/// Pura. Não escreve em lugar nenhum — o chamador debita `plano.investimento` do caixa e
/// soma os três deltas.
pub fn planejar_desenvolvimento(
    entrada: &EntradaDeDesenvolvimento,
    p: &ParametrosDeDesenvolvimento,
) -> PlanoDeDesenvolvimento {
    let excedente = entrada.excedente(p);
    let apetite = entrada.apetite.clamp(0.0, 1.0);
    let mut investimento = excedente * p.fracao_do_excedente.max(0.0) * apetite;
    if let Some(teto) = p.teto_do_investimento {
        investimento = investimento.min(teto.max(0.0) * entrada.custo_operacional_anual.max(0.0));
    }

    let pontos = pontos_por_investimento(investimento, entrada.custo_operacional_anual, p);

    // A depreciação morde a estrutura ATUAL, não a comprada: ficar no topo custa mais que
    // ficar no meio, e é isso que impede a catraca.
    let deprecia = |atual: f64| atual.clamp(0.0, 100.0) * p.depreciacao_anual.max(0.0);
    let (dep_eng, dep_ins, dep_con) = (
        deprecia(entrada.engenharia),
        deprecia(entrada.instalacoes),
        deprecia(entrada.confiabilidade),
    );

    // Delta bruto por eixo, e depois o clamp na faixa 0–100 para que o chamador possa somar
    // sem pensar.
    let limitar = |atual: f64, delta: f64| -> f64 {
        let atual = atual.clamp(0.0, 100.0);
        (atual + delta).clamp(0.0, 100.0) - atual
    };

    PlanoDeDesenvolvimento {
        excedente,
        investimento,
        pontos_comprados: pontos,
        pontos_depreciados: dep_eng + dep_ins + dep_con,
        delta_engenharia: limitar(entrada.engenharia, pontos * PESO_ENGENHARIA - dep_eng),
        delta_instalacoes: limitar(entrada.instalacoes, pontos * PESO_INSTALACOES - dep_ins),
        delta_confiabilidade: limitar(
            entrada.confiabilidade,
            pontos * PESO_CONFIABILIDADE - dep_con,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn equipe(caixa: f64) -> EntradaDeDesenvolvimento {
        EntradaDeDesenvolvimento {
            caixa,
            divida: 0.0,
            custo_operacional_anual: 1_000_000.0,
            engenharia: 55.0,
            instalacoes: 55.0,
            confiabilidade: 65.0,
            apetite: 1.0,
        }
    }

    /// A regra que a medição impôs: quem não tem excedente não paga. É o que mantém a GT3
    /// (superávit medido de −22,5% do operacional anual) intocada pelo ralo.
    #[test]
    fn equipe_sem_excedente_nao_investe() {
        let p = ParametrosDeDesenvolvimento::default();
        // Caixa abaixo da reserva de 12 meses.
        let plano = planejar_desenvolvimento(&equipe(400_000.0), &p);
        assert_eq!(plano.investimento, 0.0);
        assert_eq!(plano.pontos_comprados, 0.0);
        // E ainda assim ela DEPRECIA: não investir tem preço.
        assert!(plano.delta_liquido() < 0.0);
    }

    /// O ponto que separa este modelo do de hoje: rico e pobre não crescem igual.
    #[test]
    fn rico_compra_mais_estrutura_que_pobre() {
        let p = ParametrosDeDesenvolvimento::default();
        let pobre = planejar_desenvolvimento(&equipe(1_200_000.0), &p);
        let rico = planejar_desenvolvimento(&equipe(6_000_000.0), &p);
        assert!(
            rico.pontos_comprados > pobre.pontos_comprados * 2.0,
            "rico {:.2} pontos vs pobre {:.2}",
            rico.pontos_comprados,
            pobre.pontos_comprados
        );
    }

    /// Retorno decrescente: quadruplicar o investimento não quadruplica o que ele compra.
    #[test]
    fn o_retorno_satura() {
        let p = ParametrosDeDesenvolvimento::default();
        let um = pontos_por_investimento(350_000.0, 1_000_000.0, &p);
        let quatro = pontos_por_investimento(1_400_000.0, 1_000_000.0, &p);
        assert!(quatro < um * 2.0, "um {um:.2} · quatro {quatro:.2}");
        assert!(quatro < p.pontos_maximos_por_temporada);
    }

    /// O dinheiro sai mesmo quando a estrutura não tem para onde subir. Sem isto o ralo
    /// secaria exatamente na equipe mais rica, que é onde ele precisa funcionar.
    #[test]
    fn estrutura_no_teto_ainda_drena_caixa() {
        let p = ParametrosDeDesenvolvimento::default();
        let mut e = equipe(20_000_000.0);
        e.engenharia = 100.0;
        e.instalacoes = 100.0;
        e.confiabilidade = 100.0;
        let plano = planejar_desenvolvimento(&e, &p);
        assert!(plano.investimento > 0.0);
        // Ela paga só para compensar a depreciação — o ganho líquido é ~zero, não positivo.
        assert!(
            plano.delta_liquido() <= 0.001,
            "{:?}",
            plano.delta_liquido()
        );
    }

    /// Somar os deltas nunca tira a estrutura da faixa 0–100.
    #[test]
    fn deltas_respeitam_a_faixa() {
        let p = ParametrosDeDesenvolvimento::default();
        for caixa in [0.0, 500_000.0, 5_000_000.0, 100_000_000.0] {
            for nivel in [0.0, 1.0, 50.0, 99.5, 100.0] {
                let mut e = equipe(caixa);
                e.engenharia = nivel;
                e.instalacoes = nivel;
                e.confiabilidade = nivel;
                let plano = planejar_desenvolvimento(&e, &p);
                for (atual, delta) in [
                    (e.engenharia, plano.delta_engenharia),
                    (e.instalacoes, plano.delta_instalacoes),
                    (e.confiabilidade, plano.delta_confiabilidade),
                ] {
                    let final_ = atual + delta;
                    assert!(
                        (0.0..=100.0).contains(&final_),
                        "caixa {caixa} nível {nivel}: {atual} + {delta} = {final_}"
                    );
                }
            }
        }
    }

    /// O teto de investimento faz o que o relatório previu: acima dele o excedente para de
    /// ser drenado. É a razão de o padrão ser `None`.
    #[test]
    fn teto_deixa_excedente_de_fora() {
        let sem_teto = ParametrosDeDesenvolvimento::default();
        let com_teto = ParametrosDeDesenvolvimento {
            teto_do_investimento: Some(0.5),
            ..sem_teto
        };
        let rica = equipe(30_000_000.0);
        let a = planejar_desenvolvimento(&rica, &sem_teto);
        let b = planejar_desenvolvimento(&rica, &com_teto);
        assert!(b.investimento < a.investimento);
        assert_eq!(b.investimento, 0.5 * rica.custo_operacional_anual);
    }

    /// Dívida não vira excedente: quem deve mais do que tem não investe.
    #[test]
    fn divida_come_o_excedente() {
        let p = ParametrosDeDesenvolvimento::default();
        let mut e = equipe(3_000_000.0);
        assert!(planejar_desenvolvimento(&e, &p).investimento > 0.0);
        e.divida = 2_500_000.0;
        assert_eq!(planejar_desenvolvimento(&e, &p).investimento, 0.0);
    }
}
