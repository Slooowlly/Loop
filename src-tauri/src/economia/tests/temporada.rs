//! Faixas e forma dos recorrentes de temporada e do custo operacional anual.

use super::legado::{amplitude_legada, operacional_legado};
use crate::economia::ancora::DIVISOES;
use crate::economia::temporada::{self, EquipeNaTemporada};
use crate::economia::tipos::Bloco;

/// Banda declarada do custo operacional ANUAL de referência — a âncora da seção 3.2.
///
/// Escopo ALL-IN: eventos, estrutura e a folha de referência da dupla de pilotos. É o mesmo
/// escopo do `operating_cost_midpoint` antigo, que também é all-in (`finance::salary`
/// projeta a folha de pilotos como 15% dele). Sem os pilotos aqui a comparação com ele
/// mediria coisas diferentes.
const ANUAL: &[(&str, Option<&str>, f64, f64)] = &[
    // Operação de clube com 4 pessoas mais dois pilotos: a folha somada é ~⅔ da conta e o
    // resto é galpão, van e seguro.
    ("mazda_rookie", None, 175_000.0, 265_000.0),
    ("toyota_rookie", None, 175_000.0, 265_000.0),
    ("mazda_amador", None, 325_000.0, 490_000.0),
    ("toyota_amador", None, 330_000.0, 495_000.0),
    ("bmw_m2", None, 560_000.0, 845_000.0),
    ("production_challenger", Some("mazda"), 530_000.0, 800_000.0),
    (
        "production_challenger",
        Some("toyota"),
        535_000.0,
        805_000.0,
    ),
    ("production_challenger", Some("bmw"), 595_000.0, 900_000.0),
    ("gt4", None, 1_210_000.0, 1_830_000.0),
    // Uma GT3 de cliente com dois carros, 24 pessoas e dois pilotos: ~3,4 milhões por ano
    // bate com o que uma equipe de campeonato nacional de GT3 gasta sem desenvolvimento.
    ("gt3", None, 2_790_000.0, 4_220_000.0),
    ("lmp2", None, 3_490_000.0, 5_280_000.0),
    ("endurance", Some("gt4"), 1_295_000.0, 1_960_000.0),
    ("endurance", Some("gt3"), 2_640_000.0, 4_000_000.0),
    ("endurance", Some("lmp2"), 3_540_000.0, 5_350_000.0),
];

/// A folha de pilotos que sai do modelo tem que ficar perto dos 15% que
/// `finance::salary::DEFAULT_TEAM_SALARY_SHARE_OF_OPERATING` projeta.
///
/// Não é calibração — os salários da tabela foram escritos por mercado, não por fração — é
/// conferência de ESCOPO. Se as duas divergissem muito, a comparação com o midpoint antigo
/// estaria de novo medindo coisas diferentes, que é o erro que esta rodada corrigiu.
#[test]
fn a_folha_de_pilotos_fica_perto_dos_quinze_por_cento_do_projeto() {
    for (categoria, classe) in DIVISOES {
        let d = temporada::decomposicao_anual(categoria, classe);
        let fatia = d.folha_de_pilotos / d.total();
        assert!(
            (0.08..0.18).contains(&fatia),
            "{categoria}:{classe:?} tem {:.1}% de folha de piloto, longe dos 15% do projeto",
            fatia * 100.0
        );
    }
}

#[test]
fn toda_divisao_tem_banda_anual() {
    assert_eq!(ANUAL.len(), DIVISOES.len());
    for (categoria, classe) in DIVISOES {
        assert!(
            ANUAL
                .iter()
                .any(|(c, k, _, _)| *c == categoria && *k == classe),
            "{categoria}:{classe:?} sem banda anual"
        );
    }
}

#[test]
fn custo_operacional_anual_dentro_da_faixa() {
    for (categoria, classe, min, max) in ANUAL {
        let anual = temporada::custo_operacional_anual_de_referencia(categoria, *classe);
        assert!(
            anual >= *min && anual <= *max,
            "{categoria}:{classe:?} deu {anual:.0}, fora de [{min:.0}, {max:.0}]"
        );
    }
}

/// A fatura de temporada é `quantidade × preço`, igual à da etapa, e mora toda no bloco
/// de estrutura.
#[test]
fn a_fatura_de_temporada_fecha_e_e_toda_estrutura() {
    let equipe = EquipeNaTemporada::default();
    for (categoria, classe) in DIVISOES {
        let f = temporada::fatura_de_temporada(categoria, classe, &equipe);
        assert!(!f.linhas.is_empty());
        for l in &f.linhas {
            assert_eq!(l.bloco, Bloco::Estrutura);
            assert!((l.total() - l.quantidade * l.preco_unitario).abs() < 1e-9);
            assert!(
                l.total() > 0.0,
                "linha '{}' com valor zero na fatura",
                l.chave
            );
        }
        assert_eq!(f.total(), f.total_do_bloco(Bloco::Estrutura));
    }
}

/// A folha é a maior linha do ano em toda divisão — é ela que carrega a escada.
#[test]
fn a_folha_tecnica_e_a_maior_linha_em_toda_divisao() {
    let equipe = EquipeNaTemporada::default();
    for (categoria, classe) in DIVISOES {
        let f = temporada::fatura_de_temporada(categoria, classe, &equipe);
        let folha = f.valor(temporada::FOLHA_TECNICA);
        for l in &f.linhas {
            assert!(
                l.chave == temporada::FOLHA_TECNICA || l.total() < folha,
                "{categoria}:{classe:?}: '{}' passou a folha",
                l.chave
            );
        }
        assert!(folha / f.total() > 0.5);
    }
}

/// Estrutura maior custa mais todo ano — o freio que a seção 3.4 pede. Crescer as
/// instalações não é de graça nem quando ninguém está melhorando nada.
#[test]
fn instalacoes_maiores_custam_mais_todo_ano() {
    let pequena =
        temporada::fatura_de_temporada("gt3", None, &EquipeNaTemporada { instalacoes: 0.0 });
    let media = temporada::fatura_de_temporada("gt3", None, &EquipeNaTemporada::default());
    let grande =
        temporada::fatura_de_temporada("gt3", None, &EquipeNaTemporada { instalacoes: 100.0 });

    assert!(pequena.valor(temporada::SEDE) < media.valor(temporada::SEDE));
    assert!(media.valor(temporada::SEDE) < grande.valor(temporada::SEDE));
    // Só a sede sente — folha e frota não mudam por causa do tamanho do galpão.
    assert_eq!(
        pequena.valor(temporada::FOLHA_TECNICA),
        grande.valor(temporada::FOLHA_TECNICA)
    );
}

/// Saúde financeira em MESES DE OPERAÇÃO, não em fração de um caixa-médio inventado.
#[test]
fn meses_de_operacao_mede_folego_real() {
    let anual = temporada::custo_operacional_anual_de_referencia("gt3", None);
    assert!((temporada::meses_de_operacao(anual, "gt3", None) - 12.0).abs() < 1e-6);
    assert!((temporada::meses_de_operacao(anual / 2.0, "gt3", None) - 6.0).abs() < 1e-6);

    // O mesmo caixa dá muito menos fôlego numa categoria mais cara — que é justamente o
    // que a fração de caixa-médio não conseguia dizer sem uma segunda tabela.
    let caixa = 1_000_000.0;
    assert!(
        temporada::meses_de_operacao(caixa, "mazda_rookie", None)
            > temporada::meses_de_operacao(caixa, "gt3", None) * 5.0
    );
}

// ── A comparação com a âncora antiga, agora TOTAL contra TOTAL ───────────────────────

/// O fator de ponte de 0,62 morreu. Ele existia porque o lado novo só tinha eventos e o
/// midpoint antigo cobria o ano inteiro; agora os dois lados cobrem o mesmo escopo e a
/// comparação é direta.
///
/// O que ela mostra: a divergência aponta para o mesmo lado na escada toda — o ano físico
/// custa MENOS que a tabela velha reservava — mas o tamanho do buraco varia por mais de
/// dez vezes entre a base e o topo. Problema de forma, não de nível.
#[test]
fn a_divergencia_com_a_ancora_antiga_varia_por_categoria() {
    let razoes: Vec<f64> = DIVISOES
        .iter()
        .map(|(c, k)| {
            temporada::custo_operacional_anual_de_referencia(c, *k) / operacional_legado(c)
        })
        .collect();

    let menor = razoes.iter().cloned().fold(f64::MAX, f64::min);
    let maior = razoes.iter().cloned().fold(0.0, f64::max);

    assert!(
        maior / menor > 5.0,
        "a razão com a âncora antiga ficou quase constante ({menor:.2}–{maior:.2}) — \
         isso é sinal de calibração contra ela, que é o que o redesign proíbe"
    );
    // E o sinal da divergência TROCA ao longo da escada, que é a forma mais forte de dizer
    // que a tabela velha está torta e não só grande: a base custa MAIS do que ela reservava
    // e o topo custa uma fração dela.
    assert!(
        maior > 1.0,
        "a base da escada deveria estourar o midpoint antigo (maior {maior:.2})"
    );
    assert!(
        menor < 0.20,
        "o topo da escada deveria divergir muito mais que a base (menor {menor:.2})"
    );
}

/// A escada física, medida no ANO INTEIRO, continua muito mais comprimida que a velha.
/// Este é o resultado que muda o jogo: a pirâmide do Loop é de ~20×, não de ~90×.
#[test]
fn a_escada_anual_e_muito_mais_comprimida_que_a_antiga() {
    let anuais: Vec<f64> = DIVISOES
        .iter()
        .map(|(c, k)| temporada::custo_operacional_anual_de_referencia(c, *k))
        .collect();
    let nova = anuais.iter().cloned().fold(0.0, f64::max)
        / anuais.iter().cloned().fold(f64::MAX, f64::min);
    let antiga = amplitude_legada();

    assert!(
        (14.0..30.0).contains(&nova),
        "a escada anual deu {nova:.1}×"
    );
    assert!(
        nova < antiga / 3.0,
        "nova {nova:.1}× vs antiga {antiga:.1}×"
    );
}
