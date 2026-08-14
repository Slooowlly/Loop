//! A suíte do monitor de corrida, ESPELHANDO os submódulos que ela cobre.
//!
//! Ela era um arquivo só de duas mil linhas enquanto o módulo sob teste já estava fatiado
//! em treze. Achar o teste de uma área virava busca textual, e um caso novo ia parar onde
//! coubesse — o que faz a suíte crescer sem que ninguém veja onde está o buraco.
//!
//! Agora cada arquivo aqui responde por um submódulo de [`super`], com uma exceção
//! deliberada: [`quali_destruida`] é uma REGRA (o castigo por carro destruído na
//! classificação), não um submódulo, e ficaria escondida no meio dos testes de medição da
//! quali. Ela atravessa `quali`, `pontuacao` e `quebras`, e é o pedaço com mais decisão de
//! produto por linha da família — merece a porta com o próprio nome.
//!
//! Os ajudantes usados por mais de um arquivo estão em [`comum`]; o resto mora ao lado do
//! caso que o explica.

mod comum;

mod amostrador;
mod estado_agora;
mod historico;
mod pontuacao;
mod quali;
mod quali_destruida;
mod quebras;
mod resultado;
mod sessao;
mod tentativas;
