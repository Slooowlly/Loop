//! **Qual classe responde por um campeonato multi-classe quando o chamador não sabe a classe.**
//!
//! Dois campeonatos do Loop são multi-classe — `endurance` (gt4/gt3/lmp2) e
//! `production_challenger` (mazda/toyota/bmw) —, e três lugares diferentes precisavam
//! responder à mesma pergunta quando só recebiam a categoria:
//!
//! | quem pergunta | o que respondia | o que quer saber |
//! |---|---|---|
//! | `finance::planning::category_finance_scale_for` | `gt3` | quanto custa operar |
//! | `car::cost::category_ceiling_for` | o teto 8 (= `lmp2`) | até onde a peça sobe |
//! | `finance::planning::representative_division_for_tier` | `lmp2` | quanto se paga |
//!
//! Cada um tinha a justificativa escrita ao lado, e **cada uma estava certa** — o problema
//! nunca foi a escolha, foi ela ser invisível de fora. Um call site novo que esquecesse a
//! classe recebia uma das três ao acaso, sem nada no tipo ou no nome indicando qual.
//!
//! Aqui a escolha vira uma pergunta explícita. Quem chama declara se quer a divisão TÍPICA
//! (a que descreve a operação média do campeonato) ou a de ÁPICE (a mais cara e mais alta),
//! e as duas respostas moram uma ao lado da outra em vez de espalhadas por três arquivos.
//!
//! **Isto não substitui a classe de verdade.** Ela mora em `Team::classe`, e quem a tem em
//! mãos deve passá-la — as formas `*_for(categoria, classe)` existem por isso. Toda resposta
//! daqui é aproximação com perda, declarada.

/// A pergunta que se está fazendo ao pedir a classe de referência.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClasseDeReferencia {
    /// **A classe TÍPICA**: a que descreve a operação média do campeonato. É a resposta certa
    /// para dinheiro de operação — o orçamento de "uma equipe de Endurance" genérica é o de
    /// uma GT3, não o de um protótipo, que é o extremo caro da grade.
    Tipica,
    /// **A classe de ÁPICE**: a mais alta da grade. É a resposta certa para teto de peça (o
    /// limite da categoria é o do carro mais capaz que ela admite) e para salário (o mercado
    /// de um tier é o do trabalho mais caro que ele oferece).
    ///
    /// No salário a escolha é estrutural, não estética: com a classe TÍPICA no tier 6, o GT3
    /// do Endurance passava a custar menos que uma operação de LMP2 do tier 5 e a escada
    /// salarial perdia a monotonicidade que `contract::salary_range_for_tier` assevera.
    Apice,
}

/// A classe que responde pelo campeonato. `None` para os campeonatos de classe única — lá a
/// pergunta não se coloca, e responder qualquer coisa seria inventar uma classe que não
/// existe.
///
/// `production_challenger` responde `bmw` às duas perguntas: as três classes são carros
/// distintos sem uma ordem de capacidade declarada em lugar nenhum do domínio, e `bmw` já era
/// a resposta única que os dois consumidores davam. Se algum dia a ordem existir, é aqui que
/// ela se escreve.
pub fn classe_de_referencia(categoria: &str, pergunta: ClasseDeReferencia) -> Option<&'static str> {
    match (categoria.trim(), pergunta) {
        ("endurance", ClasseDeReferencia::Tipica) => Some("gt3"),
        ("endurance", ClasseDeReferencia::Apice) => Some("lmp2"),
        ("production_challenger", _) => Some("bmw"),
        _ => None,
    }
}

/// Os campeonatos que têm classe. Existe para os testes varrerem o conjunto em vez de
/// repetirem a lista à mão.
pub const CAMPEONATOS_MULTI_CLASSE: [&str; 2] = ["endurance", "production_challenger"];

#[cfg(test)]
mod tests {
    use super::*;

    /// Todo campeonato multi-classe responde às DUAS perguntas, e nenhum de classe única
    /// responde a qualquer uma. Sem isto, acrescentar um campeonato multi-classe deixaria um
    /// buraco silencioso.
    #[test]
    fn todo_campeonato_multi_classe_responde_as_duas_perguntas() {
        for campeonato in CAMPEONATOS_MULTI_CLASSE {
            for pergunta in [ClasseDeReferencia::Tipica, ClasseDeReferencia::Apice] {
                let classe = classe_de_referencia(campeonato, pergunta)
                    .unwrap_or_else(|| panic!("{campeonato} sem resposta para {pergunta:?}"));
                assert!(
                    crate::constants::categories::is_valid_competitive_division(
                        campeonato,
                        Some(classe)
                    ),
                    "{campeonato}:{classe} não é uma divisão válida"
                );
            }
        }
        for solteiro in [
            "gt3",
            "gt4",
            "lmp2",
            "bmw_m2",
            "mazda_rookie",
            "inexistente",
        ] {
            assert_eq!(
                classe_de_referencia(solteiro, ClasseDeReferencia::Tipica),
                None
            );
            assert_eq!(
                classe_de_referencia(solteiro, ClasseDeReferencia::Apice),
                None
            );
        }
    }

    /// **As respostas que os três consumidores davam antes continuam valendo.** Este teste é
    /// o contrato da unificação: ela não podia mover número nenhum, só tornar visível qual
    /// pergunta cada um faz.
    #[test]
    fn a_unificacao_preserva_as_respostas_historicas() {
        // `finance::planning` operava com a típica.
        assert_eq!(
            classe_de_referencia("endurance", ClasseDeReferencia::Tipica),
            Some("gt3")
        );
        // `car::cost` mantinha o teto histórico 8, que é o do lmp2.
        assert_eq!(
            crate::car::cost::category_ceiling_for("endurance", None),
            crate::car::cost::category_ceiling_for("endurance", Some("lmp2"))
        );
        // O tier salarial de topo resolvia para o ápice.
        assert_eq!(
            crate::finance::planning::representative_division_for_tier(6),
            ("endurance", Some("lmp2"))
        );
        // E a típica é MAIS BARATA que o ápice — é o que separa as duas perguntas.
        let tipica = crate::finance::planning::category_finance_scale_for("endurance", Some("gt3"));
        let apice = crate::finance::planning::category_finance_scale_for("endurance", Some("lmp2"));
        assert!(
            tipica.operating_cost_max < apice.operating_cost_max,
            "típica {} deveria custar menos que o ápice {}",
            tipica.operating_cost_max,
            apice.operating_cost_max
        );
    }
}
