#![allow(dead_code)]

// INTEGRACAO FUTURA:
// driver.rs::generate_for_category() deve chamar generate_pilot_identity()
// para obter nome, nacionalidade e genero realistas.
// Hoje driver.rs usa nomes placeholder - sera atualizado quando o Wizard for implementado.

//! Fachada dos nomes de piloto. A implementacao vive em `names/`:
//! - `dados`: as listas de nomes e sobrenomes por regiao
//! - `pool`: a tabela `NamePool` por nacionalidade e seus acessores
//! - `identidade`: sorteio de nome, genero e identidade completa

mod dados;
mod identidade;
mod pool;

pub use identidade::*;
pub use pool::*;

#[cfg(test)]
mod tests;
