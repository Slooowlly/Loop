//! Economia do Loop, versão bottom-up.
//!
//! Módulo novo, escrito ao lado do antigo (`finance/`) e ainda **não integrado**: nada
//! aqui é chamado pela simulação. Ele nasce isolado para poder ser medido antes de
//! substituir alguma coisa.
//!
//! A diferença de fundo para `finance/` está no sentido da conta. Lá, cada linha da fatura
//! é uma fração de um orçamento por categoria escrito à mão
//! (`finance::planning::category_finance_scale`) e depois batizada com o nome de um objeto
//! físico — daí a linha "gasolina" de uma etapa de BMW M2 custar ~9.400 e a de Endurance
//! ~188.000, números que não têm relação nenhuma com a coisa que o rótulo nomeia. Aqui não
//! existe orçamento de entrada: existem quilômetros, litros, jogos de pneu e pessoas, e o
//! custo é o que essas quantidades somam.
//!
//! Consequência aceita de propósito: o custo operacional que sai daqui **não bate** com a
//! tabela antiga, e não deve bater. A tabela antiga é o que está sob suspeita.
//!
//! Estado atual (etapas 2 e 3 da Parte 6 do redesign):
//!
//! | arquivo | papel |
//! |---|---|
//! | [`tipos`] | entrada e saída, puros — sem `rusqlite`, sem Tauri |
//! | [`ancora`] | os parâmetros físicos por divisão, numa tabela só |
//! | [`evento`] | a fatura de uma etapa |
//!
//! | [`temporada`] | os recorrentes do ano e o **custo operacional anual de referência** |
//!
//! A âncora da seção 3.2 é [`temporada::custo_operacional_anual_de_referencia`]: eventos
//! mais estrutura, uma consequência do modelo físico. Ainda não existem `receita` (os cinco
//! canais) e `estado` — embora [`temporada::meses_de_operacao`] já entregue a medida de
//! saúde que o `estado` vai usar.

// O módulo é uma fundação que ainda ninguém chama — a integração com `race/persistencia.rs`
// é uma etapa posterior do plano. Sem isto o crate inteiro vira um muro de `never used`, e
// a fachada abaixo (que existe justamente para essa integração) conta como import morto.
#![allow(dead_code, unused_imports)]

pub mod ancora;
pub mod desenvolvimento;
pub mod evento;
pub mod fatura;
pub mod receita;
pub mod temporada;
pub mod tipos;

/// A fachada: o que `race/persistencia.rs` vai chamar quando a troca da seção 3.9 acontecer.
pub use evento::fatura_da_etapa;
pub use tipos::{Bloco, EntradaDaEtapa, FaturaDaEtapa, LinhaDaFatura, Unidade};

/// A fachada da APRESENTAÇÃO: o que a tela do pós-corrida vai consumir. Separada da
/// fachada do modelo de propósito — o modelo é detalhado, a fatura agrega.
pub use fatura::{fatura_visivel, BlocoDaFatura, EntradaDaFatura, FaturaVisivel, LinhaVisivel};

#[cfg(test)]
mod tests;
