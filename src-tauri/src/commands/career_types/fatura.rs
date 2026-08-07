//! DTOs da FATURA DA ETAPA — a prestação de contas do fim de semana que o jogador lê.
//!
//! Espelho serde de [`crate::economia::fatura::FaturaVisivel`]. Existe separado porque o
//! módulo de economia é lógica pura e não conhece serde nem camelCase, e porque o que
//! cruza a ponte carrega uma coisa que o modelo puro não tem: o bloco de CONSERTO, que é
//! evento de jogo (a batida do jogador) e não conta física de operação.
//!
//! **Só tokens.** Nenhum campo aqui é texto de UI: `chave`, `bloco` e `unidade` são
//! tokens de i18n que o React resolve em `raceResult.invoice.*` e `maintenance.*`. É o que
//! permite a mesma fatura ler em pt-BR e en-US sem o Rust saber qual está ativo.

use serde::{Deserialize, Serialize};

use crate::economia::fatura::{Detalhe, FaturaVisivel, LinhaVisivel};

/// Uma linha do modelo interno atrás de uma linha visível: a quantidade física e o preço
/// unitário que a produzem. É o que o "expandir" mostra.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InvoiceDetailDto {
    /// Token da linha interna (`folha_tecnica`, `sede`, `viagem`…).
    pub key: String,
    pub quantity: f64,
    /// Token da unidade (`litro`, `pessoa_noite`, `pessoa_ano`…).
    pub unit: String,
    pub unit_price: f64,
    /// `quantity × unit_price`. Vem calculado para a tela não ter que repetir a conta —
    /// e para a tela não PODER discordar dela.
    pub total: f64,
}

impl From<&Detalhe> for InvoiceDetailDto {
    fn from(d: &Detalhe) -> Self {
        Self {
            key: d.chave.to_string(),
            quantity: d.quantidade,
            unit: d.unidade.chave().to_string(),
            unit_price: d.preco_unitario,
            total: d.total(),
        }
    }
}

/// Uma linha visível da fatura.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InvoiceLineDto {
    /// Token da linha (`combustivel`, `viagem_e_estadia`, `bilheteria`…).
    pub key: String,
    /// Token do bloco (`corrida`, `logistica`, `equipe`, `receita`, `reparo`).
    pub block: String,
    pub total: f64,
    pub detail: Vec<InvoiceDetailDto>,
    /// A tela deve oferecer o expandir? Verdadeiro quando a linha agrega mais de uma
    /// coisa ou quando é um rateio (o detalhe está numa escala diferente da linha).
    pub expandable: bool,
    /// Por quantas etapas o detalhe se divide. 1 em tudo que a etapa consumiu; o número
    /// de rodadas do campeonato no rodapé do custo fixo.
    pub divisor: f64,
}

impl InvoiceLineDto {
    fn from_linha(l: &LinhaVisivel) -> Self {
        Self {
            key: l.chave.to_string(),
            block: l.bloco.chave().to_string(),
            total: l.total(),
            detail: l.detalhe.iter().map(InvoiceDetailDto::from).collect(),
            expandable: l.tem_detalhe(),
            divisor: l.divisor,
        }
    }
}

/// Bloco do conserto. Não vem de `economia`: a batida é evento de jogo, não consumo de
/// operação, e o débito dela é cobrado num caminho próprio.
pub const INVOICE_BLOCK_REPAIR: &str = "reparo";

/// A fatura de uma etapa como o jogador a lê.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StageInvoiceDto {
    /// Linhas de despesa e de receita, já na ordem dos blocos.
    pub lines: Vec<InvoiceLineDto>,
    /// O RODAPÉ: o custo fixo do ano inteiro (folha técnica, sede, frota, seguros).
    ///
    /// **Não está em `lines` e não entra em `expense_total`** — decisão 10. Folha e sede
    /// não variam por corrida, então mostrá-las por corrida é falsa precisão. O `total`
    /// desta linha é a fatia da etapa e `annualTotal` é o ano inteiro, que é o número que
    /// o rodapé diz.
    pub fixed_cost: Option<InvoiceLineDto>,
    /// O custo fixo do ANO inteiro. 0 quando não há rodapé.
    pub fixed_cost_annual: f64,
    /// Soma das linhas de despesa da ETAPA — exatamente o `event_operations_cost` que
    /// saiu do caixa nesta rodada, mais o conserto se houve.
    pub expense_total: f64,
    pub income_total: f64,
    /// `income_total − expense_total`. Positivo = a etapa se pagou.
    pub result: f64,
    /// Só o que a batida cobrou. 0 num fim de semana limpo — a tela usa isto para decidir
    /// se pinta o total de alerta ou de custo de rotina.
    pub repair_total: f64,
    /// Quantas rodadas o campeonato tem. Contexto do rodapé.
    pub rounds_in_season: f64,
}

impl StageInvoiceDto {
    /// Converte a fatura pura, acrescentando o bloco de conserto que o jogo cobra à parte.
    ///
    /// O conserto entra como linhas do bloco `reparo`, sem detalhe físico: a divisão por
    /// peça é uma curadoria de severidade (`manutencao::damage_split`), não uma medição —
    /// e inventar "1 × $4.200" para ela seria escrever uma grandeza que não existe.
    pub fn from_fatura(
        fatura: &FaturaVisivel,
        rounds_in_season: f64,
        reparo: &[(String, f64)],
    ) -> Self {
        let mut lines: Vec<InvoiceLineDto> =
            fatura.linhas.iter().map(InvoiceLineDto::from_linha).collect();

        let repair_total: f64 = reparo.iter().map(|(_, v)| *v).filter(|v| *v > 0.0).sum();
        for (key, valor) in reparo.iter().filter(|(_, v)| *v > 0.0) {
            lines.push(InvoiceLineDto {
                key: key.clone(),
                block: INVOICE_BLOCK_REPAIR.to_string(),
                total: *valor,
                detail: Vec::new(),
                expandable: false,
                divisor: 1.0,
            });
        }

        let expense_total = fatura.total_de_despesa() + repair_total;
        let income_total = fatura.total_de_receita();
        Self {
            lines,
            fixed_cost: fatura
                .custo_fixo_do_ano
                .as_ref()
                .map(InvoiceLineDto::from_linha),
            fixed_cost_annual: fatura.custo_fixo_anual(),
            expense_total,
            income_total,
            result: income_total - expense_total,
            repair_total,
            rounds_in_season,
        }
    }
}
