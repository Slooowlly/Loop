//! Fonte **única** de "o que o export do iRacing sabe fazer com uma categoria da carreira".
//!
//! Existiam três regras de categoria→carro rodando ao mesmo tempo: uma no frontend
//! (`carKeyForCategory`, por substring), uma na pintura (`car_key_for_category`, a mesma
//! substring copiada) e nenhuma no roster/season, que simplesmente confiavam na `car_key`
//! que o frontend mandasse. As três terminavam no MESMO `else`: qualquer categoria que não
//! casasse "gr86"/"toyota"/"bmw"/"m2" virava `mx5`. Na prática GT4, GT3, Production
//! Challenger e Endurance eram exportados como Mazda MX-5 sem que nada acusasse — o grid
//! saía num carro que não é o da categoria, e o jogador só descobria dentro do simulador.
//!
//! Aqui a decisão é uma só, explícita e por identidade da categoria. O que o export não
//! sabe fazer hoje é **recusado com motivo**, nunca aproximado: escolher um carro pago ou
//! um substituto para GT4/GT3/LMP2/Production/Endurance é decisão de produto, e ela não
//! está tomada. Fail-closed aqui é o lado seguro de errar — um export que não acontece é
//! visível, um export com o carro errado não é.

use crate::car::breakdown::DuracaoDeProva;

/// Por que uma categoria do catálogo não pode ser exportada hoje.
///
/// Cada variante é uma decisão de produto pendente diferente, e é por isso que elas não se
/// fundem num "não suportado" só: o que destrava GT3 (escolher um carro) não é o que
/// destrava Production Challenger (representar mais de uma classe no mesmo aiseason).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MotivoDaRecusa {
    /// A string nem é uma categoria do catálogo (save corrompido, chamada manual errada).
    ForaDoCatalogo,
    /// O carro da categoria não é conteúdo grátis e nenhum substituto foi autorizado.
    CarroNaoDecidido,
    /// Grid de mais de uma classe. O aiseason declara UM `car_id`/`car_class_id` para a
    /// temporada inteira, então exportar multi-classe hoje seria escolher uma das classes
    /// e apagar as outras.
    MultiClasse,
    /// A duração muda de etapa para etapa. O aiseason tem UM `race_length` para a
    /// temporada inteira (ver [`duracao_da_temporada`]).
    DuracaoVariavelPorEtapa,
}

/// Uma recusa de export: o motivo tipado, para o teste e para quem chama, e o texto pronto
/// para a tela. O texto é fixo de propósito — ele diz ao jogador o que falta decidir, e
/// essa frase não deve variar por caminho de chamada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecusaDeExport {
    pub motivo: MotivoDaRecusa,
    pub mensagem: &'static str,
}

/// O que o export sabe fazer com uma categoria.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuporteDeExport {
    /// Exporta com este carro. A chave é a de
    /// [`crate::iracing_sdk::roster_gen::car_spec`], e o teste
    /// `todo_carro_do_catalogo_existe_no_car_spec` trava as duas listas juntas.
    Carro(&'static str),
    Recusado(RecusaDeExport),
}

const RECUSA_FORA_DO_CATALOGO: RecusaDeExport = RecusaDeExport {
    motivo: MotivoDaRecusa::ForaDoCatalogo,
    mensagem: "Categoria desconhecida — não há como saber qual carro exportar para o iRacing.",
};

const RECUSA_CARRO_NAO_DECIDIDO: RecusaDeExport = RecusaDeExport {
    motivo: MotivoDaRecusa::CarroNaoDecidido,
    mensagem: "Esta categoria ainda não tem carro definido para o iRacing. O carro dela não \
               está no conteúdo grátis, e exportar num carro aproximado colocaria o grid \
               inteiro num modelo que não é o da categoria. Corra a etapa pela simulação do \
               Loop até a escolha do carro ser feita.",
};

const RECUSA_MULTI_CLASSE: RecusaDeExport = RecusaDeExport {
    motivo: MotivoDaRecusa::MultiClasse,
    mensagem: "Esta categoria corre com mais de uma classe no mesmo grid, e o formato de AI \
               season do iRacing aceita um carro só por temporada. Exportar hoje significaria \
               escolher uma das classes e apagar as outras. Corra a etapa pela simulação do \
               Loop.",
};

const RECUSA_DURACAO_VARIAVEL: RecusaDeExport = RecusaDeExport {
    motivo: MotivoDaRecusa::DuracaoVariavelPorEtapa,
    mensagem: "As etapas desta temporada têm durações diferentes, e o formato de AI season do \
               iRacing aceita uma duração só para o campeonato inteiro. Exportar assim faria \
               as etapas longas correrem com o tempo da curta. Corra a etapa pela simulação do \
               Loop.",
};

/// O que o export sabe fazer com `categoria`.
///
/// O `match` é sobre a IDENTIDADE da categoria, nunca sobre substring: `"gt3"` contendo
/// `"gt"` ou um nome de equipe contendo `"bmw"` não decidem carro nenhum. Toda entrada do
/// catálogo aparece por nome, e o braço final recusa em vez de escolher um padrão — é ele
/// que substitui o `else → mx5`.
pub fn suporte_de_export(categoria: &str) -> SuporteDeExport {
    match categoria.trim() {
        "mazda_rookie" | "mazda_amador" => SuporteDeExport::Carro("mx5"),
        "toyota_rookie" | "toyota_amador" => SuporteDeExport::Carro("gr86"),
        "bmw_m2" => SuporteDeExport::Carro("bmwm2"),
        // Grid de três classes cada uma. Além disso o Endurance tem carro pago e duração
        // por etapa; o bloqueio que aparece primeiro é o da classe.
        "production_challenger" | "endurance" => SuporteDeExport::Recusado(RECUSA_MULTI_CLASSE),
        // Carro pago e sem substituto autorizado. `lmp2` não está em `CATEGORIES` (é classe
        // de referência dentro do Endurance), mas `get_category_config` a resolve, então ela
        // entra aqui em vez de cair no braço de fora do catálogo.
        "gt4" | "gt3" | "lmp2" => SuporteDeExport::Recusado(RECUSA_CARRO_NAO_DECIDIDO),
        _ => SuporteDeExport::Recusado(RECUSA_FORA_DO_CATALOGO),
    }
}

/// A chave de carro da categoria, ou a mensagem de recusa pronta para a tela.
///
/// É a porta que roster, season e pintura usam. Nenhuma delas volta a receber `car_key` de
/// fora: quem manda de fora manda a categoria, e a tradução acontece uma vez, aqui.
pub fn car_key_da_categoria(categoria: &str) -> Result<&'static str, String> {
    match suporte_de_export(categoria) {
        SuporteDeExport::Carro(chave) => Ok(chave),
        SuporteDeExport::Recusado(recusa) => Err(recusa.mensagem.to_string()),
    }
}

/// A duração que o aiseason pode declarar para a temporada inteira.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DuracaoDaTemporada {
    /// Todas as etapas duram o mesmo — este valor vira o `race_length` do arquivo.
    Uniforme(DuracaoDeProva),
    /// As etapas divergem. O formato não representa isso sem perda.
    VariaPorEtapa { menor: u16, maior: u16 },
}

/// Reduz as durações EFETIVAS das etapas ao único `race_length` que o aiseason aceita.
///
/// A entrada é [`DuracaoDeProva`], não `u16`: a sentinela `0` da categoria já morreu na
/// cascata de [`crate::calendar::duracao_efetiva`], então nenhum zero chega aqui e
/// nenhum zero sai. É esse encadeamento que garante o `race_length` diferente de zero no
/// arquivo, em vez de uma checagem defensiva que alguém esquece de repetir.
///
/// `None` para lista vazia: temporada sem etapa não tem duração, e inventar uma seria
/// exatamente o que este módulo existe para impedir.
pub fn duracao_da_temporada(etapas: &[DuracaoDeProva]) -> Option<DuracaoDaTemporada> {
    let menor = etapas.iter().min()?.minutos();
    let maior = etapas.iter().max()?.minutos();
    if menor == maior {
        // `menor` vem de uma `DuracaoDeProva`, então nunca é a sentinela.
        Some(DuracaoDaTemporada::Uniforme(DuracaoDeProva::constante(
            menor,
        )))
    } else {
        Some(DuracaoDaTemporada::VariaPorEtapa { menor, maior })
    }
}

/// A duração única da temporada, ou a mensagem de recusa pronta para a tela.
///
/// A mensagem da divergência carrega os dois extremos medidos: sem eles a recusa vira um
/// "não dá" sem informação, e o jogador não tem como saber qual etapa está fora da linha.
pub fn race_length_da_temporada(etapas: &[DuracaoDeProva]) -> Result<DuracaoDeProva, String> {
    match duracao_da_temporada(etapas) {
        Some(DuracaoDaTemporada::Uniforme(d)) => Ok(d),
        Some(DuracaoDaTemporada::VariaPorEtapa { menor, maior }) => Err(format!(
            "{} (as etapas vão de {menor} a {maior} minutos)",
            RECUSA_DURACAO_VARIAVEL.mensagem
        )),
        None => Err("Nenhuma etapa para exportar nesta temporada.".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::categories::get_all_categories;
    use crate::iracing_sdk::roster_gen::car_spec;

    /// As categorias que o export sabe fazer hoje, com o carro de cada uma. Lista literal
    /// de propósito: ela é o contrato, e mudá-la precisa ser uma edição consciente.
    const SUPORTADAS: [(&str, &str); 5] = [
        ("mazda_rookie", "mx5"),
        ("toyota_rookie", "gr86"),
        ("mazda_amador", "mx5"),
        ("toyota_amador", "gr86"),
        ("bmw_m2", "bmwm2"),
    ];

    /// O critério de aceite em um teste: **todo** id do catálogo é decidido, e o que não é
    /// suportado é RECUSADO, não aproximado. Uma categoria nova entra por aqui — ou ela
    /// ganha carro na tabela, ou o motivo dela precisa ser escrito.
    #[test]
    fn o_catalogo_inteiro_e_mapeado_ou_recusado_explicitamente() {
        let mut ids: Vec<&str> = get_all_categories().iter().map(|c| c.id).collect();
        // `lmp2` é classe de referência: não está em `CATEGORIES`, mas `get_category_config`
        // a resolve, então o export pode ser chamado com ela.
        ids.push("lmp2");

        for id in ids {
            let esperado = SUPORTADAS.iter().find(|(cat, _)| *cat == id);
            match (suporte_de_export(id), esperado) {
                (SuporteDeExport::Carro(chave), Some((_, esperada))) => {
                    assert_eq!(chave, *esperada, "{id} mapeou para o carro errado");
                }
                (SuporteDeExport::Carro(chave), None) => {
                    panic!("{id} não é suportada mas mapeou para {chave}");
                }
                (SuporteDeExport::Recusado(recusa), None) => {
                    assert_ne!(
                        recusa.motivo,
                        MotivoDaRecusa::ForaDoCatalogo,
                        "{id} está no catálogo e não pode ser recusada como desconhecida"
                    );
                    assert!(
                        !recusa.mensagem.trim().is_empty(),
                        "{id} recusou sem motivo"
                    );
                }
                (SuporteDeExport::Recusado(_), Some((_, esperada))) => {
                    panic!("{id} deveria exportar em {esperada} e foi recusada");
                }
            }
        }
    }

    /// O bug que este módulo fecha: nada mais cai em MX-5 por omissão.
    #[test]
    fn nenhuma_categoria_nao_suportada_cai_em_mx5() {
        for categoria in [
            "gt4",
            "gt3",
            "lmp2",
            "production_challenger",
            "endurance",
            "categoria_que_nao_existe",
            "",
        ] {
            assert!(
                matches!(suporte_de_export(categoria), SuporteDeExport::Recusado(_)),
                "{categoria} escapou como carro suportado"
            );
            assert!(car_key_da_categoria(categoria).is_err());
        }
    }

    /// A decisão é por identidade. Substring era exatamente a regra antiga, e ela mapeava
    /// pelo nome errado: `bmw_m2_endurance` não existe hoje, mas casava "bmw" e viraria M2.
    #[test]
    fn a_decisao_nao_e_por_substring() {
        for parecida in ["gt3_bmw", "toyota_endurance", "mazda", "bmw", "m2", "gr86"] {
            assert!(
                matches!(suporte_de_export(parecida), SuporteDeExport::Recusado(_)),
                "{parecida} não é id de categoria e não pode decidir carro"
            );
        }
    }

    /// As categorias que já funcionavam continuam com o MESMO carro de antes.
    #[test]
    fn as_categorias_suportadas_mantem_o_carro_de_sempre() {
        for (categoria, chave) in SUPORTADAS {
            assert_eq!(car_key_da_categoria(categoria).unwrap(), chave);
        }
    }

    /// A chave que a tabela devolve precisa existir do outro lado, senão a recusa vira erro
    /// de "carro desconhecido" lá na frente, sem dizer o que houve.
    #[test]
    fn todo_carro_do_catalogo_existe_no_car_spec() {
        for (categoria, _) in SUPORTADAS {
            let chave = car_key_da_categoria(categoria).unwrap();
            assert!(
                car_spec(chave).is_some(),
                "{categoria} aponta para {chave}, que não está em roster_gen::car_spec"
            );
        }
    }

    #[test]
    fn temporada_de_etapas_iguais_tem_duracao_unica() {
        let etapas = vec![DuracaoDeProva::constante(25); 8];
        assert_eq!(
            duracao_da_temporada(&etapas),
            Some(DuracaoDaTemporada::Uniforme(DuracaoDeProva::constante(25)))
        );
        assert_eq!(race_length_da_temporada(&etapas).unwrap().minutos(), 25);
    }

    /// O caso do Endurance: 120, 180, 240 e 360 minutos sorteados por etapa. Escolher um
    /// deles é decisão de balanceamento, então o export recusa em vez de escolher.
    #[test]
    fn temporada_de_etapas_diferentes_e_recusada_com_os_extremos() {
        let etapas: Vec<DuracaoDeProva> = [120, 360, 180, 240]
            .iter()
            .map(|m| DuracaoDeProva::constante(*m))
            .collect();
        assert_eq!(
            duracao_da_temporada(&etapas),
            Some(DuracaoDaTemporada::VariaPorEtapa {
                menor: 120,
                maior: 360
            })
        );
        let erro = race_length_da_temporada(&etapas).unwrap_err();
        assert!(
            erro.contains("120"),
            "a recusa precisa dizer o menor: {erro}"
        );
        assert!(
            erro.contains("360"),
            "a recusa precisa dizer o maior: {erro}"
        );
    }

    #[test]
    fn temporada_sem_etapa_nao_inventa_duracao() {
        assert_eq!(duracao_da_temporada(&[]), None);
        assert!(race_length_da_temporada(&[]).is_err());
    }

    /// O invariante que sustenta o critério de aceite "nenhum artefato com `race_length` 0".
    /// A entrada é varrida a partir do que o banco pode gravar, passando pela cascata que
    /// resolve a sentinela — e o que sai do redutor nunca é zero.
    #[test]
    fn o_race_length_nunca_sai_zero() {
        use crate::calendar::duracao_efetiva;
        for categoria in get_all_categories().iter().map(|c| c.id) {
            for bruto in [-5, 0, 15, 25, 120, 360] {
                let d = duracao_efetiva(bruto, categoria);
                let unica = race_length_da_temporada(&[d]).expect("etapa única é uniforme");
                assert!(
                    unica.minutos() > 0,
                    "{categoria} com bruto {bruto} produziu race_length zero"
                );
            }
        }
    }
}
