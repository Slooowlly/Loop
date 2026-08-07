//! A fatura de UMA etapa, calculada de baixo para cima.
//!
//! Toda linha daqui é `quantidade × preço_unitário`. Não existe uma constante que diga
//! "combustível é 7% do orçamento" — existe quantos quilômetros os carros rodaram, quantos
//! litros isso pede e quanto custa o litro. A soma das linhas É o custo da etapa; nenhum
//! número precisou saber de antemão quanto a etapa "deveria" custar.
//!
//! O que a etapa concreta muda em relação à âncora:
//!
//! - **abandono** encurta o combustível e o pneu de corrida, mas não devolve o que treino
//!   e classificação já queimaram;
//! - **pneu castigado** consome mais borracha que pneu poupado;
//! - **distância da pista** move frete, passagem e estadia;
//! - **estrutura e pit crew** movem o volume transportado e o tamanho da comitiva.
//!
//! - **duração da prova** move combustível, pneu, revisão, treino e a taxa de inscrição.
//!
//! O que a etapa concreta NÃO muda: o preço do litro, o preço do jogo de pneu, a diária de
//! hotel. Esses são preços de mercado, e é por isso que a fatura fica legível.

use super::ancora::{
    self, ParametrosFisicos, DIARIA_DE_HOTEL, DIARIA_DE_PESSOAL, FRETE_FATOR_LONGA_DISTANCIA,
    FRETE_KM_SEM_DESCONTO, PASSAGEM_PARTE_FIXA, PASSAGEM_POR_KM, PRECO_LITRO_COMBUSTIVEL,
};
use super::tipos::{Bloco, EntradaDaEtapa, FaturaDaEtapa, LinhaDaFatura, Unidade};

// ── Tokens das linhas ────────────────────────────────────────────────────────────────
pub const COMBUSTIVEL: &str = "combustivel";
pub const PNEUS: &str = "pneus";
pub const REVISAO: &str = "revisao";
pub const FRETE: &str = "frete";
pub const VIAGEM: &str = "viagem";
pub const ESTADIA: &str = "estadia";
pub const INSCRICAO: &str = "inscricao";
pub const DIARIAS: &str = "diarias";

/// Quilômetros que a transportadora fatura: ida e volta, com desconto de longa distância
/// acima de [`FRETE_KM_SEM_DESCONTO`].
pub fn km_faturados_de_frete(distancia_ida_km: f64) -> f64 {
    let d = distancia_ida_km.max(0.0);
    let trecho_cheio = d.min(FRETE_KM_SEM_DESCONTO);
    let trecho_longo = (d - FRETE_KM_SEM_DESCONTO).max(0.0) * FRETE_FATOR_LONGA_DISTANCIA;
    (trecho_cheio + trecho_longo) * 2.0
}

/// Preço da passagem de uma pessoa para esta etapa.
pub fn preco_da_passagem(distancia_ida_km: f64) -> f64 {
    PASSAGEM_PARTE_FIXA + PASSAGEM_POR_KM * distancia_ida_km.max(0.0)
}

/// Escala de 0,85 a 1,15 em cima de um atributo 0–100, neutra em 50. Serve para o tamanho
/// da comitiva (pit crew) e para o volume transportado (instalações): é uma variação de
/// PORTE em torno da operação típica, não um multiplicador de orçamento.
fn escala_de_porte(atributo: f64) -> f64 {
    0.85 + atributo.clamp(0.0, 100.0) / 333.0
}

/// A fatura da etapa.
pub fn fatura_da_etapa(entrada: &EntradaDaEtapa) -> FaturaDaEtapa {
    let p = ancora::parametros(&entrada.categoria, entrada.classe.as_deref());
    let carros = entrada.equipe.carros_inscritos as f64;
    if carros <= 0.0 {
        return FaturaDaEtapa { linhas: vec![] };
    }

    let distancia = entrada.pista.distancia_da_sede_km;

    // ── Bloco CORRIDA ────────────────────────────────────────────────────────────────
    // Quilometragem de verdade: o que os carros rodaram na corrida mais o que o fim de
    // semana consumiu antes dela. Treino e classificação já aconteceram — abandonar na
    // primeira volta não devolve esse combustível. O programa de treinos acompanha a
    // duração da prova, mas de forma amortecida.
    let km_de_corrida = entrada.corrida.voltas_completadas.max(0.0) * entrada.pista.comprimento_km;
    let km_treino_quali = p.km_treino_quali_em(entrada.duracao_corrida_min);
    let km_por_carro = km_de_corrida + km_treino_quali;
    let km_da_equipe = km_por_carro * carros;

    let litros = km_da_equipe * p.consumo_l_por_km;

    // Jogos de pneu, em duas parcelas de naturezas diferentes:
    //   - os de treino e quali são gastos de qualquer jeito;
    //   - os de corrida saem da distância que o carro EFETIVAMENTE rodou, dividida pela
    //     vida de um jogo. Não existe mais fração de "corrida completa": a quilometragem
    //     é a grandeza, e ela já sabe se o carro abandonou ou se a prova era de 6 horas.
    // O desgaste com que o carro voltou desloca só a parcela de corrida — em desgaste
    // típico (0,65) o fator é ~1,0 e a etapa de referência reproduz o nominal da tabela.
    let castigo_do_pneu = 0.90 + 0.15 * entrada.corrida.desgaste_final_pneus.clamp(0.0, 1.0);
    let jogos_de_corrida = if p.km_por_jogo_de_corrida > 0.0 {
        km_de_corrida / p.km_por_jogo_de_corrida * castigo_do_pneu
    } else {
        0.0
    };
    let jogos = (p.jogos_de_pneu_fixos + jogos_de_corrida) * carros;

    // ── Bloco LOGÍSTICA ──────────────────────────────────────────────────────────────
    // Instalações melhores significam mais volume na estrada (box montável maior,
    // motorhome, hospitalidade), não uma etapa proporcionalmente mais cara.
    let km_de_frete =
        km_faturados_de_frete(distancia) * escala_de_porte(entrada.equipe.instalacoes);

    // A comitiva é gente: arredonda para pessoa inteira, e nunca abaixo de duas (alguém
    // precisa pilotar e alguém precisa receber o carro).
    let comitiva = (p.comitiva * escala_de_porte(entrada.equipe.qualidade_pit_crew))
        .round()
        .max(2.0);

    // ── Bloco EQUIPE ─────────────────────────────────────────────────────────────────
    // Dias trabalhados = noites + 1: chega na véspera, vai embora depois da prova.
    let dias = p.noites_de_hotel + 1.0;

    let linhas = vec![
        LinhaDaFatura::nova(
            COMBUSTIVEL,
            Bloco::Corrida,
            litros,
            Unidade::Litro,
            PRECO_LITRO_COMBUSTIVEL,
        ),
        LinhaDaFatura::nova(
            PNEUS,
            Bloco::Corrida,
            jogos,
            Unidade::JogoDePneu,
            p.preco_do_jogo_de_pneu,
        ),
        LinhaDaFatura::nova(
            REVISAO,
            Bloco::Corrida,
            km_da_equipe,
            Unidade::Km,
            p.revisao_por_km,
        ),
        LinhaDaFatura::nova(
            FRETE,
            Bloco::Logistica,
            km_de_frete,
            Unidade::KmDeFrete,
            p.tarifa_frete_por_km,
        ),
        LinhaDaFatura::nova(
            VIAGEM,
            Bloco::Logistica,
            comitiva,
            Unidade::Pessoa,
            preco_da_passagem(distancia),
        ),
        LinhaDaFatura::nova(
            ESTADIA,
            Bloco::Logistica,
            comitiva * p.noites_de_hotel,
            Unidade::PessoaNoite,
            DIARIA_DE_HOTEL,
        ),
        LinhaDaFatura::nova(
            INSCRICAO,
            Bloco::Logistica,
            carros,
            Unidade::Carro,
            p.taxa_de_inscricao_em(entrada.duracao_corrida_min),
        ),
        LinhaDaFatura::nova(
            DIARIAS,
            Bloco::Equipe,
            comitiva * dias,
            Unidade::PessoaDia,
            DIARIA_DE_PESSOAL,
        ),
    ];

    FaturaDaEtapa { linhas }
}

/// Custo de operação de UMA TEMPORADA de eventos para uma equipe mediana da divisão.
///
/// É a soma das faturas típicas das etapas do campeonato, sob uma mistura de viagem
/// realista — não toda etapa é continental. Este número é a CONSEQUÊNCIA do modelo
/// físico: é o que a seção 3.2 do redesign chama de custo operacional de referência, e
/// existe para ser comparado com a âncora antiga, não para ser calibrado contra ela.
///
/// Não inclui folha fixa, sede, frota nem seguro — esses são recorrentes de temporada e
/// moram em [`crate::economia::temporada`]. O custo operacional anual de referência é a
/// soma dos dois, e mora lá.
pub fn custo_de_eventos_da_temporada(categoria: &str, classe: Option<&str>) -> f64 {
    custo_de_eventos_da_temporada_com(categoria, classe, &super::tipos::EquipeNaEtapa::default())
}

/// O mesmo, para uma equipe de porte escolhido. Serve para achar as PONTAS da faixa: uma
/// equipe com instalações e crew no chão contra uma no teto.
pub fn custo_de_eventos_da_temporada_com(
    categoria: &str,
    classe: Option<&str>,
    equipe: &super::tipos::EquipeNaEtapa,
) -> f64 {
    let etapas = ancora::etapas_por_temporada(categoria);

    // Mistura de viagem de um calendário: uma etapa em casa, uma intercontinental (só nas
    // categorias com calendário longo o bastante para ter uma), o resto continental.
    let intercontinentais = if etapas >= 8.0 { 1.0 } else { 0.0 };
    let em_casa = 1.0_f64.min(etapas);
    let continentais = (etapas - em_casa - intercontinentais).max(0.0);

    // Custo esperado de uma etapa a esta distância, médio sobre as durações que o
    // calendário pode sortear. Nas categorias de sprint há uma duração só e a média é ela
    // mesma; no endurance são quatro, equiprováveis.
    let custo_com = |distancia: f64| {
        let duracoes = duracoes_possiveis(categoria, classe);
        let soma: f64 = duracoes
            .iter()
            .map(|&d| {
                let mut e = super::tipos::EntradaDaEtapa::tipica_com_duracao(categoria, classe, d);
                e.pista.distancia_da_sede_km = distancia;
                e.equipe = *equipe;
                fatura_da_etapa(&e).total()
            })
            .sum();
        soma / duracoes.len() as f64
    };

    custo_com(super::tipos::DISTANCIA_CASA_KM) * em_casa
        + custo_com(super::tipos::DISTANCIA_CONTINENTAL_KM) * continentais
        + custo_com(super::tipos::DISTANCIA_INTERCONTINENTAL_KM) * intercontinentais
}

/// As durações de prova que o calendário pode sortear para a categoria, em minutos.
///
/// Espelha `calendar::montagem::resolve_race_duration`: categoria com
/// `duracao_corrida_min > 0` tem uma duração só; o endurance tem `0` na constante e sorteia
/// entre quatro valores com peso igual. Sem isto a temporada de endurance seria orçada numa
/// prova média que nunca acontece.
pub fn duracoes_possiveis(categoria: &str, classe: Option<&str>) -> Vec<f64> {
    match crate::constants::categories::get_category_config(categoria) {
        Some(c) if c.duracao_corrida_min > 0 => vec![c.duracao_corrida_min as f64],
        Some(_) => vec![120.0, 180.0, 240.0, 360.0],
        None => vec![ancora::parametros(categoria, classe).duracao_corrida_min],
    }
}

/// Parâmetros da divisão, reexportado para quem precisa da tabela sem passar pela fatura.
pub fn parametros_da_divisao(categoria: &str, classe: Option<&str>) -> ParametrosFisicos {
    ancora::parametros(categoria, classe)
}
