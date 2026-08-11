//! Economia do Loop, versão bottom-up.
//!
//! Módulo novo, escrito ao lado do antigo (`finance/`) e **integrado**. Ele nasceu isolado
//! para poder ser medido antes de substituir alguma coisa, e a substituição aconteceu:
//!
//! | quem consome hoje | o que consome |
//! |---|---|
//! | `finance::planning` | [`temporada::faixa_de_caixa`], [`temporada::faixa_de_custo_operacional_anual`] — a ÂNCORA da escala financeira por divisão |
//! | `finance::state` | a mesma âncora, pela medida de meses de operação |
//! | `commands::race::despesa` | [`ancora`], [`temporada`], [`receita`], [`evento::fatura_da_etapa`] |
//! | `commands::race::fatura` | [`fatura::fatura_visivel`] — a fatura da tela de pós-corrida |
//! | `market::preseason::inicializacao` | [`desenvolvimento::planejar_desenvolvimento`] — o offseason de toda equipe do mundo |
//! | `finance::planning`, `car::cost` | [`divisao::classe_de_referencia`] — a classe que responde por um campeonato multi-classe |
//!
//! O último a entrar foi [`desenvolvimento`], o ralo do excedente. Ele tomou o lugar de
//! `finance::cashflow::offseason::apply_offseason_competitiveness_impact`, que creditava
//! ~2,76 pontos de estrutura por equipe por temporada **sem debitar caixa** — uma das fontes
//! de dinheiro do nada do redesign, e a razão declarada de o superávit das equipes não ter
//! para onde ir. A regalia não foi apagada: virou o braço de controle do harness A/B, sob
//! `cfg(test)`, para que a calibração do modelo novo continue tendo com o que ser comparada.
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
//! Os arquivos:
//!
//! | arquivo | papel |
//! |---|---|
//! | [`tipos`] | entrada e saída, puros — sem `rusqlite`, sem Tauri |
//! | [`ancora`] | os parâmetros físicos por divisão, numa tabela só |
//! | [`evento`] | a fatura de uma etapa |
//! | [`temporada`] | os recorrentes do ano e o **custo operacional anual de referência** |
//! | [`receita`] | os cinco canais, declarados como total de temporada |
//! | [`fatura`] | a apresentação da fatura para o jogador |
//! | [`desenvolvimento`] | o ralo do excedente: o offseason das equipes |
//! | [`divisao`] | qual classe responde por um campeonato multi-classe, e para qual pergunta |
//!
//! A âncora da seção 3.2 é [`temporada::custo_operacional_anual_de_referencia`]: eventos
//! mais estrutura, uma consequência do modelo físico.

// O `allow` de módulo inteiro **saiu**. Ele existia enquanto nada aqui era chamado pela
// simulação — sem ele o crate virava um muro de `never used` —, e nesse estado escondia
// import e função mortos de verdade. Com todos os submódulos integrados, o que sobra sem
// chamador é pontual e leva o `allow` no próprio item, onde dá para ler o motivo.

pub mod ancora;
pub mod desenvolvimento;
pub mod divisao;
pub mod evento;
pub mod fatura;
pub mod receita;
pub mod temporada;
pub mod tipos;

// Não há fachada de re-export aqui. Os consumidores entram pelo caminho completo
// (`economia::evento::fatura_da_etapa`, `economia::fatura::fatura_visivel`, os tipos em
// `economia::tipos`), que é o que o código de hoje realmente usa. Re-exports de atalho
// existiram por um tempo apontando para a troca da seção 3.9, nunca ganharam consumidor e
// só produziam warning: os itens continuam públicos nos submódulos.

#[cfg(test)]
mod tests;
