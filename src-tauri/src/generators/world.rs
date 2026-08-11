#![allow(dead_code)]

//! Fachada da geracao de mundo. A implementacao vive em `world/`:
//! - `tipos`: pacotes de saida, alocador de ids e ajuste de classe no contrato
//! - `pareamento`: o laco N1/N2 + contratos, compartilhado pelos dois mundos
//! - `genesis`: mundo do ano 1 com o jogador na equipe escolhida
//! - `historico`: mundo que comeca no passado, com linha do tempo de fundacao

mod genesis;
mod historico;
mod pareamento;
mod tipos;

pub use genesis::*;
pub use historico::*;
pub use tipos::*;

#[cfg(test)]
mod tests;
