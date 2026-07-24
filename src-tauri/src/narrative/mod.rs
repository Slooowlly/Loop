#![allow(dead_code)]

//! Motor de contexto narrativo do boletim de IA.
//!
//! Transforma o resultado de uma corrida em "beats" (pedaços de história já
//! avaliados, cada um com um peso). Em seguida filtra pelo limiar de relevância
//! e renderiza um CONTEXTO CURADO — denso em narrativa, enxuto em dados — que é
//! o que será enviado ao servidor → Gemini.
//!
//! Filosofia: a inteligência de "o que é interessante" mora AQUI, não na IA.
//! A IA só redige em cima dos fatos que escolhermos (zero invenção de resultado).
//!
//! Esta é a Etapa A (MVP): só os beats que saem do próprio `RaceResult`.
//! Os beats de carreira/forma (lesão, rookie, rivalidade-arco, forma das últimas
//! 5 corridas) entram na Etapa B, alimentados pela base do app.

mod beats;
mod consulta;
mod contexto;
mod incidentes;
mod tese;

pub mod client;

#[cfg(test)]
mod tests;

pub use beats::*;
pub use consulta::*;
pub use contexto::*;
pub use incidentes::*;
pub use tese::*;
