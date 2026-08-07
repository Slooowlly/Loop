//! Os recorrentes de temporada e o **custo operacional anual de referência**.
//!
//! [`evento`](super::evento) responde "quanto custa correr uma etapa". Este módulo responde
//! "quanto custa existir um ano", e a soma dos dois é a âncora que a seção 3.2 do redesign
//! promete: `custo_operacional_anual_de_referencia`. Todo o resto da economia nova pendura
//! nesse número — e ele é uma CONSEQUÊNCIA do modelo físico, não uma tabela escrita à mão.
//!
//! ## As duas naturezas de recorrente
//!
//! A separação abaixo não é organizacional, é uma hipótese que dá para medir. Existe a
//! suspeita de que a íngreme da pirâmide do Loop venha de custos **categóricos** — coisas
//! que simplesmente não existem abaixo de certo degrau — e não de "mais gente do mesmo
//! tipo". Uma equipe de Rookie não tem uma versão barata de simulador; ela não tem
//! simulador.
//!
//! - **Escalares**: existem em toda divisão e variam de tamanho. Folha técnica, sede,
//!   frota, seguros.
//! - **Categóricos**: valem **zero** abaixo do degrau em que a coisa passa a existir, e a
//!   linha nem aparece na fatura. Suporte de fábrica, simulador, aquisição de dados.
//!
//! [`decomposicao_anual`] devolve os três montantes separados justamente para a hipótese
//! poder ser testada em vez de acreditada — ver `tests::hipotese`.
//!
//! ## Dois candidatos a categórico que foram RECUSADOS
//!
//! Ambos pareciam confirmar a hipótese e ambos teriam inflado o topo por contagem dupla.
//! Registrados aqui porque a recusa é o resultado mais importante deste módulo:
//!
//! - **Aluguel / programa de motor selado.** É real: motor de GT3 é lacrado e revisado pela
//!   fábrica em intervalo fixo, e o Gibson de LMP2 é alugado por temporada. Mas revisão de
//!   motor **já está** em [`ancora::ParametrosFisicos::revisao_por_km`], amortizada por
//!   quilômetro. Cobrar de novo aqui somaria ~200 mil ao topo da escada sem nenhum custo
//!   novo do outro lado — exatamente o viés que a hipótese está sob risco de produzir.
//! - **Motorhome e hospitalidade.** Também real e também categórico, e também já contado:
//!   está no `frota_anual` (que é o conjunto de transporte inteiro, não só o cegonha) e as
//!   pessoas de hospitalidade já estão na `comitiva` da etapa.
//!
//! E um que **não existe em nível nenhum**: túnel de vento. O Loop modela corrida de
//! cliente — carro homologado, aerodinâmica congelada por BoP. Nem a equipe de LMP2
//! desenvolve aero; o chassi é spec. Não há degrau em que essa linha nasça.

use std::collections::HashMap;

use super::ancora::{self, ParametrosFisicos};
use super::evento::custo_de_eventos_da_temporada;
use super::tipos::{Bloco, FaturaDaEtapa, LinhaDaFatura, Unidade};
use crate::constants::categories::competitive_division_key;

// ── Tokens das linhas ────────────────────────────────────────────────────────────────
pub const FOLHA_TECNICA: &str = "folha_tecnica";
pub const FOLHA_DE_PILOTOS: &str = "folha_de_pilotos";
pub const SEDE: &str = "sede";
pub const FROTA: &str = "frota";
pub const SEGURO_E_LICENCA: &str = "seguro_e_licenca";
pub const SUPORTE_DE_FABRICA: &str = "suporte_de_fabrica";
pub const SIMULADOR: &str = "simulador";
pub const AQUISICAO_DE_DADOS: &str = "aquisicao_de_dados";

/// As linhas que são categóricas — as que valem zero abaixo do degrau em que a coisa
/// existe. A lista é a definição operacional da hipótese.
pub const LINHAS_CATEGORICAS: [&str; 3] = [SUPORTE_DE_FABRICA, SIMULADOR, AQUISICAO_DE_DADOS];

pub fn e_categorica(chave: &str) -> bool {
    LINHAS_CATEGORICAS.contains(&chave)
}

/// Recorrentes anuais de UMA divisão competitiva, em dólar.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParametrosDeTemporada {
    // ── Escalares ───────────────────────────────────────────────────────────────────
    /// Salário anual médio de uma pessoa da equipe fixa. Sobe a escada porque a operação
    /// se profissionaliza: mecânico de fim de semana vira mecânico contratado, e no topo
    /// entra engenheiro de corrida, que é o cargo caro.
    pub salario_medio_anual: f64,
    /// Aluguel, energia e manutenção do galpão. Modulado pelas instalações da equipe.
    pub sede_anual: f64,
    /// Amortização e manutenção do conjunto de transporte: van, cegonha, carreta, box
    /// montável, motorhome. É o ativo por trás da `tarifa_frete_por_km` da etapa.
    pub frota_anual: f64,
    /// Seguro de responsabilidade e de casco, licença anual da equipe no campeonato e
    /// credenciais da tripulação. Sobe rápido porque o casco segurado sobe rápido.
    pub seguro_e_licenca_anual: f64,
    /// Salário anual de UM piloto, no ponto médio da divisão.
    ///
    /// Está aqui por uma razão de ESCOPO, não de simulação. O
    /// `operating_cost_midpoint` antigo é all-in: `finance::salary` deriva a base salarial
    /// dele com `DEFAULT_TEAM_SALARY_SHARE_OF_OPERATING = 0.15`, ou seja, a folha de dois
    /// pilotos é 15% do midpoint por projeto. Sem esta linha o custo anual daqui seria
    /// técnico-only e compará-lo com o midpoint compararia coisas diferentes.
    ///
    /// **Não é o que a simulação debita.** O gasto real com piloto é a folha dos contratos
    /// (`finance::salary`, `commands::race::financas`). Isto é o valor de REFERÊNCIA de uma
    /// dupla mediana, e existe para a âncora cobrir o mesmo escopo que a tabela que ela
    /// substitui.
    pub salario_medio_de_piloto_anual: f64,

    // ── Categóricos ─────────────────────────────────────────────────────────────────
    /// Contrato de suporte técnico do fabricante do carro de cliente: disponibilidade de
    /// peça, engenheiro de fábrica no evento, atualização de mapa e de software.
    /// **Zero** onde o carro não é um produto de corrida de cliente.
    pub suporte_de_fabrica_anual: f64,
    /// Dias de simulador com piloto no laço. **Zero** abaixo do GT4: não existe simulador
    /// barato, existe não ter simulador.
    pub simulador_anual: f64,
    /// Sistema profissional de telemetria e aquisição de dados: hardware, licença de
    /// software, suporte. **Zero** nas monomarcas de base, onde o carro loga o mínimo.
    pub aquisicao_de_dados_anual: f64,
}

impl ParametrosDeTemporada {
    pub fn total_categorico(&self) -> f64 {
        self.suporte_de_fabrica_anual + self.simulador_anual + self.aquisicao_de_dados_anual
    }
}

/// A equipe do ponto de vista da temporada.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EquipeNaTemporada {
    /// Instalações (0–100). Estrutura maior tem conta de sede maior — é o freio natural
    /// que a seção 3.4 pede: crescer aumenta o custo fixo de todo mês.
    pub instalacoes: f64,
}

impl Default for EquipeNaTemporada {
    fn default() -> Self {
        Self { instalacoes: 50.0 }
    }
}

/// Escala da sede pelas instalações: 0,60 em zero, 1,00 no meio da escala, 1,40 no topo.
/// Mais aberta que a escala de porte da etapa porque aqui o efeito É o tamanho do imóvel,
/// não uma variação de comitiva.
fn escala_da_sede(instalacoes: f64) -> f64 {
    0.60 + instalacoes.clamp(0.0, 100.0) / 125.0
}

/// Pilotos por equipe, de `constants::categories`. São 2 em toda a escada hoje; ler da
/// constante em vez de fixar o 2 evita que uma categoria futura de piloto solo passe
/// despercebida pela folha.
fn pilotos_por_equipe(categoria: &str) -> f64 {
    crate::constants::categories::get_category_config(categoria)
        .map(|c| c.pilotos_por_equipe as f64)
        .unwrap_or(2.0)
}

/// A tabela dos recorrentes. Uma linha por divisão competitiva, mesma chave de
/// [`ancora::parametros`].
///
/// | divisão | salário | sede | frota | seguro | fábrica | simulador | dados |
/// |---|---:|---:|---:|---:|---:|---:|---:|
/// | rookie | 28.000 | 12.000 | 12.000 | 6.000 | — | — | — |
/// | amador | 28.000 | 20.000 | 18.000 | 10.000 | — | — | — |
/// | bmw / production | 38.000 | 35.000 | 32.000 | 18.000 | 15.000 | — | 8.000 |
/// | gt4 | 52.000 | 60.000 | 55.000 | 40.000 | 45.000 | 25.000 | 30.000 |
/// | gt3 | 68.000 | 95.000 | 110.000 | 85.000 | 120.000 | 90.000 | 70.000 |
/// | lmp2 | 78.000 | 120.000 | 135.000 | 110.000 | 90.000 | 110.000 | 85.000 |
/// | endurance | = classe | = classe | +15% | +30% | = classe | = classe | = classe |
pub fn parametros_de_temporada(categoria: &str, classe: Option<&str>) -> ParametrosDeTemporada {
    match competitive_division_key(categoria, classe).as_str() {
        // ── Tier 0 e 1 — monomarca de base ───────────────────────────────────────────
        // Operação de clube: o galpão é uma unidade de ~200 m², a frota é uma van com
        // reboque, e o mecânico é um profissional júnior de oficina.
        //
        // Nenhum categórico existe aqui, e essa é a afirmação forte da tabela: não há
        // simulador barato, não há contrato de fábrica pequeno, não há telemetria
        // simplificada. O carro é um MX-5 de campeonato monomarca; a equipe abre o capô e
        // olha.
        "mazda_rookie" | "toyota_rookie" => ParametrosDeTemporada {
            salario_medio_anual: 28_000.0,
            sede_anual: 12_000.0,
            frota_anual: 12_000.0,
            seguro_e_licenca_anual: 6_000.0,
            salario_medio_de_piloto_anual: 15_000.0,
            suporte_de_fabrica_anual: 0.0,
            simulador_anual: 0.0,
            aquisicao_de_dados_anual: 0.0,
        },
        "mazda_amador" | "toyota_amador" => ParametrosDeTemporada {
            salario_medio_anual: 28_000.0,
            // Galpão maior: dois carros preparados a sério e estoque de peça.
            sede_anual: 20_000.0,
            frota_anual: 18_000.0,
            seguro_e_licenca_anual: 10_000.0,
            salario_medio_de_piloto_anual: 25_000.0,
            suporte_de_fabrica_anual: 0.0,
            simulador_anual: 0.0,
            aquisicao_de_dados_anual: 0.0,
        },

        // ── Tier 2 — BMW M2 e Production ─────────────────────────────────────────────
        // É AQUI que nascem dois categóricos, e o degrau é o próprio carro: o M2 CS Racing
        // é um produto de corrida de cliente vendido pela BMW M Motorsport, com contrato de
        // peça e suporte. E é o primeiro carro do jogo que sai de fábrica com aquisição de
        // dados de verdade — abaixo dele o dado não existe, não é mais barato.
        "bmw_m2"
        | "production_challenger:mazda"
        | "production_challenger:toyota"
        | "production_challenger:bmw" => ParametrosDeTemporada {
            salario_medio_anual: 38_000.0,
            sede_anual: 35_000.0,
            // Caminhão-baú próprio em vez de reboque atrás da van.
            frota_anual: 32_000.0,
            seguro_e_licenca_anual: 18_000.0,
            salario_medio_de_piloto_anual: 45_000.0,
            suporte_de_fabrica_anual: 15_000.0,
            // Simulador ainda não: nesta categoria ninguém compra dia de simulador.
            simulador_anual: 0.0,
            aquisicao_de_dados_anual: 8_000.0,
        },

        // ── Tier 3 — GT4 ─────────────────────────────────────────────────────────────
        // O degrau do SIMULADOR. GT4 é a primeira categoria do jogo em que a equipe compra
        // dia de simulador com piloto no laço para preparar pista — e não existe meia dose
        // disso.
        "gt4" | "endurance:gt4" => {
            let enduro = classe.is_some();
            ParametrosDeTemporada {
                // Entra engenheiro de pista contratado, não só mecânico.
                salario_medio_anual: 52_000.0,
                sede_anual: 60_000.0,
                // Carreta de corrida com dois carros e box montável.
                // Endurance carrega mais equipamento: +15%.
                frota_anual: if enduro { 65_000.0 } else { 55_000.0 },
                // Prova longa é mais risco de casco: +30%.
                seguro_e_licenca_anual: if enduro { 55_000.0 } else { 40_000.0 },
                salario_medio_de_piloto_anual: 80_000.0,
                suporte_de_fabrica_anual: 45_000.0,
                simulador_anual: 25_000.0,
                aquisicao_de_dados_anual: 30_000.0,
            }
        }

        // ── Tier 4 — GT3 ─────────────────────────────────────────────────────────────
        // Sem degrau categórico novo: o GT3 tem as MESMAS três linhas do GT4, maiores.
        // Isso é dado contra a hipótese, e está aqui de propósito — a alternativa seria
        // inventar uma linha que só nasce no GT3 para confirmar o que se quer provar.
        "gt3" | "endurance:gt3" => {
            let enduro = classe.is_some();
            ParametrosDeTemporada {
                // Dois engenheiros de corrida (~110 mil) diluídos em 24 pessoas.
                salario_medio_anual: 68_000.0,
                // Galpão de 800–1.200 m² com baias de preparação e oficina.
                sede_anual: 95_000.0,
                frota_anual: if enduro { 125_000.0 } else { 110_000.0 },
                // Casco de GT3 é um ativo de meio milhão de dólares.
                seguro_e_licenca_anual: if enduro { 110_000.0 } else { 85_000.0 },
                salario_medio_de_piloto_anual: 180_000.0,
                suporte_de_fabrica_anual: 120_000.0,
                simulador_anual: 90_000.0,
                aquisicao_de_dados_anual: 70_000.0,
            }
        }

        // ── Tier 5 — LMP2 ────────────────────────────────────────────────────────────
        // Chassi spec (Oreca e afins): o suporte do construtor é MENOR que o de um
        // programa de fábrica de GT3, porque não há marca disputando campeonato de marcas.
        // A conta que sobe é a de gente.
        "lmp2" | "endurance:lmp2" => {
            let enduro = classe.is_some();
            ParametrosDeTemporada {
                salario_medio_anual: 78_000.0,
                sede_anual: 120_000.0,
                frota_anual: if enduro { 150_000.0 } else { 135_000.0 },
                seguro_e_licenca_anual: if enduro { 140_000.0 } else { 110_000.0 },
                salario_medio_de_piloto_anual: 250_000.0,
                suporte_de_fabrica_anual: 90_000.0,
                simulador_anual: 110_000.0,
                aquisicao_de_dados_anual: 85_000.0,
            }
        }

        // Divisão desconhecida cai no meio da escada, igual à âncora física.
        _ => parametros_de_temporada("bmw_m2", None),
    }
}

/// A fatura anual dos recorrentes. Linha categórica de valor zero **não aparece** — é a
/// diferença entre "custa pouco" e "não existe", e a fatura tem que dizer qual dos dois é.
pub fn fatura_de_temporada(
    categoria: &str,
    classe: Option<&str>,
    equipe: &EquipeNaTemporada,
) -> FaturaDaEtapa {
    let fisico: ParametrosFisicos = ancora::parametros(categoria, classe);
    let p = parametros_de_temporada(categoria, classe);

    let linhas = [
        LinhaDaFatura::nova(
            FOLHA_TECNICA,
            Bloco::Estrutura,
            fisico.equipe_fixa,
            Unidade::PessoaAno,
            p.salario_medio_anual,
        ),
        LinhaDaFatura::nova(
            FOLHA_DE_PILOTOS,
            Bloco::Estrutura,
            pilotos_por_equipe(categoria),
            Unidade::PessoaAno,
            p.salario_medio_de_piloto_anual,
        ),
        LinhaDaFatura::nova(
            SEDE,
            Bloco::Estrutura,
            1.0,
            Unidade::Ano,
            p.sede_anual * escala_da_sede(equipe.instalacoes),
        ),
        LinhaDaFatura::nova(FROTA, Bloco::Estrutura, 1.0, Unidade::Ano, p.frota_anual),
        LinhaDaFatura::nova(
            SEGURO_E_LICENCA,
            Bloco::Estrutura,
            1.0,
            Unidade::Ano,
            p.seguro_e_licenca_anual,
        ),
        LinhaDaFatura::nova(
            SUPORTE_DE_FABRICA,
            Bloco::Estrutura,
            1.0,
            Unidade::Ano,
            p.suporte_de_fabrica_anual,
        ),
        LinhaDaFatura::nova(
            SIMULADOR,
            Bloco::Estrutura,
            1.0,
            Unidade::Ano,
            p.simulador_anual,
        ),
        LinhaDaFatura::nova(
            AQUISICAO_DE_DADOS,
            Bloco::Estrutura,
            1.0,
            Unidade::Ano,
            p.aquisicao_de_dados_anual,
        ),
    ]
    .into_iter()
    .filter(|l| l.total() > 0.0)
    .collect();

    FaturaDaEtapa { linhas }
}

/// Como o custo anual se reparte. Existe para a hipótese dos custos categóricos poder ser
/// medida em vez de acreditada.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DecomposicaoAnual {
    /// Soma das faturas das etapas do campeonato.
    pub eventos: f64,
    /// Recorrentes técnicos que existem em toda divisão: folha técnica, sede, frota,
    /// seguro. **Não** inclui piloto.
    pub recorrentes_escalares: f64,
    /// Recorrentes que não existem abaixo do degrau em que nascem.
    pub recorrentes_categoricos: f64,
    /// Folha de referência da dupla de pilotos. Separada porque é a parcela que decide se
    /// a comparação com `operating_cost_midpoint` é de escopos iguais — o midpoint antigo
    /// é all-in.
    pub folha_de_pilotos: f64,
}

impl DecomposicaoAnual {
    /// O custo anual ALL-IN — o escopo que o `operating_cost_midpoint` antigo cobre, e o
    /// único número que pode ser comparado com ele.
    pub fn total(&self) -> f64 {
        self.total_sem_pilotos() + self.folha_de_pilotos
    }

    /// O custo anual da operação TÉCNICA, sem piloto. É o recorte da primeira versão desta
    /// medição; fica exposto para os dois números poderem ser lidos lado a lado.
    pub fn total_sem_pilotos(&self) -> f64 {
        self.eventos + self.recorrentes_escalares + self.recorrentes_categoricos
    }

    /// Quanto do ano é categórico. É o número que a hipótese precisa que seja grande.
    pub fn fracao_categorica(&self) -> f64 {
        let total = self.total();
        if total <= 0.0 {
            return 0.0;
        }
        self.recorrentes_categoricos / total
    }
}

/// A decomposição do ano de uma equipe MEDIANA da divisão.
pub fn decomposicao_anual(categoria: &str, classe: Option<&str>) -> DecomposicaoAnual {
    let equipe = EquipeNaTemporada::default();
    let fatura = fatura_de_temporada(categoria, classe, &equipe);

    let categoricos: f64 = fatura
        .linhas
        .iter()
        .filter(|l| e_categorica(l.chave))
        .map(LinhaDaFatura::total)
        .sum();
    let pilotos = fatura.valor(FOLHA_DE_PILOTOS);

    DecomposicaoAnual {
        eventos: custo_de_eventos_da_temporada(categoria, classe),
        recorrentes_escalares: sem_zero_negativo(fatura.total() - categoricos - pilotos),
        recorrentes_categoricos: sem_zero_negativo(categoricos),
        folha_de_pilotos: sem_zero_negativo(pilotos),
    }
}

/// Troca `-0.0` por `0.0`. As divisões de base não têm linha categórica nenhuma e a soma
/// vazia pode sair com sinal negativo — aí o relatório imprime "-0" e parece bug de sinal
/// onde só existe ausência. `f64::max` não serve aqui: com (-0,0) e (+0,0) ela pode
/// devolver qualquer um dos dois.
fn sem_zero_negativo(valor: f64) -> f64 {
    if valor == 0.0 {
        0.0
    } else {
        valor
    }
}

/// **A âncora da seção 3.2**: o que uma equipe mediana desta divisão gasta numa temporada,
/// sem desenvolvimento de carro.
///
/// Eventos mais recorrentes, e nada mais. Não inclui salário de piloto (que é contrato, e
/// já existe em `finance::salary`), nem melhoria de estrutura (que é
/// [`crate::economia::desenvolvimento`]), nem juros de dívida.
///
/// Este número **substitui** `finance::planning::CategoryFinanceScale::operating_cost_midpoint`
/// como referência — e diverge dele de propósito. A tabela velha é o que está sob suspeita;
/// nada aqui foi escolhido para chegar perto dela.
pub fn custo_operacional_anual_de_referencia(categoria: &str, classe: Option<&str>) -> f64 {
    decomposicao_anual(categoria, classe).total()
}

/// A FAIXA do custo operacional anual da divisão: `(enxuta, encorpada)`.
///
/// As pontas não são inventadas — são o mesmo modelo com a equipe nos extremos dos
/// atributos que de fato movem custo. Instalações no chão significam galpão pequeno e menos
/// volume na estrada; no teto, o contrário. Pit crew no chão significa comitiva menor.
///
/// Existe porque `finance::planning::CategoryFinanceScale` guarda `operating_cost_min` e
/// `operating_cost_max`, e as duas pontas precisam vir do modelo em vez de serem uma
/// abertura arbitrária em volta do ponto médio.
pub fn faixa_de_custo_operacional_anual(categoria: &str, classe: Option<&str>) -> (f64, f64) {
    *memo_da_faixa()
        .entry(competitive_division_key(categoria, classe))
        .or_insert_with(|| calcular_faixa_de_custo_operacional_anual(categoria, classe))
}

/// Memo das faixas por divisão.
///
/// Sem isto a troca de âncora vira um problema de desempenho: `category_finance_scale` é
/// consultada por equipe e por rodada, e cada consulta dispara a temporada inteira das duas
/// pontas — algo como cinquenta faturas de etapa por chamada. Numa geração histórica de 25
/// temporadas isso são milhões de faturas para produzir catorze números que nunca mudam.
///
/// O resultado depende só da divisão (as tabelas são constantes), então uma divisão é
/// calculada uma vez por processo.
fn memo_da_faixa() -> std::sync::MutexGuard<'static, HashMap<String, (f64, f64)>> {
    static MEMO: std::sync::LazyLock<std::sync::Mutex<HashMap<String, (f64, f64)>>> =
        std::sync::LazyLock::new(|| std::sync::Mutex::new(HashMap::new()));
    MEMO.lock().unwrap_or_else(|e| e.into_inner())
}

fn calcular_faixa_de_custo_operacional_anual(categoria: &str, classe: Option<&str>) -> (f64, f64) {
    let ponta = |atributo: f64| {
        let etapa = super::tipos::EquipeNaEtapa {
            instalacoes: atributo,
            qualidade_pit_crew: atributo,
            ..super::tipos::EquipeNaEtapa::default()
        };
        let eventos = super::evento::custo_de_eventos_da_temporada_com(categoria, classe, &etapa);
        let recorrentes = fatura_de_temporada(
            categoria,
            classe,
            &EquipeNaTemporada {
                instalacoes: atributo,
            },
        )
        .total();
        eventos + recorrentes
    };
    (ponta(0.0), ponta(100.0))
}

// ── O caixa como CONSEQUÊNCIA (seção 3.2) ────────────────────────────────────────────
//
// A tabela antiga declarava um caixa esperado por categoria — 6 a 25 milhões na GT3 — sem
// nunca dizer em que unidade isso estava. Medido, estava em MESES: `expected_cash_midpoint`
// era ~2,05× o `operating_cost_midpoint` em toda a escada, ou seja, a tabela mandava toda
// equipe guardar **~24 meses de operação** em caixa. Isso não é uma equipe de corrida de
// cliente, é um fundo.
//
// Agora a unidade é explícita e o número é uma decisão declarada.

/// Meses de operação que a equipe mais apertada da divisão consegue bancar. Um mês é a
/// equipe que vive de etapa em etapa: qualquer imprevisto vira dívida.
pub const CAIXA_MESES_MIN: f64 = 1.0;

/// Meses de operação da equipe mais capitalizada. Onze meses é quase uma temporada inteira
/// no banco — confortável de verdade, e ainda muito longe dos 24 que a tabela velha
/// implicava para a equipe MEDIANA.
pub const CAIXA_MESES_MAX: f64 = 11.0;

/// Meses de operação da equipe mediana: meia temporada guardada.
pub fn caixa_meses_de_referencia() -> f64 {
    (CAIXA_MESES_MIN + CAIXA_MESES_MAX) / 2.0
}

/// Caixa de referência da divisão — o que a equipe mediana deveria ter no banco.
///
/// É o substituto de `expected_cash_midpoint`, e a diferença é que ele não é uma constante:
/// sai do custo operacional anual, que por sua vez sai da conta física. Caixa deixou de ser
/// entrada do modelo e virou consequência dele, que é o que a seção 3.2 pede.
pub fn caixa_de_referencia(categoria: &str, classe: Option<&str>) -> f64 {
    caixa_para_meses(categoria, classe, caixa_meses_de_referencia())
}

/// A faixa de caixa da divisão: `(apertada, capitalizada)`.
pub fn faixa_de_caixa(categoria: &str, classe: Option<&str>) -> (f64, f64) {
    (
        caixa_para_meses(categoria, classe, CAIXA_MESES_MIN),
        caixa_para_meses(categoria, classe, CAIXA_MESES_MAX),
    )
}

/// Quanto caixa são `meses` de operação nesta divisão.
pub fn caixa_para_meses(categoria: &str, classe: Option<&str>, meses: f64) -> f64 {
    custo_operacional_anual_de_referencia(categoria, classe) * meses.max(0.0) / 12.0
}

/// Meses de operação que um caixa banca — a medida de saúde financeira que a seção 3.2
/// pede no lugar de "fração de um caixa-médio inventado".
pub fn meses_de_operacao(caixa: f64, categoria: &str, classe: Option<&str>) -> f64 {
    let anual = custo_operacional_anual_de_referencia(categoria, classe);
    if anual <= 0.0 {
        return 0.0;
    }
    caixa / (anual / 12.0)
}
