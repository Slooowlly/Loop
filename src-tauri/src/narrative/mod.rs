//! Motor de contexto narrativo do boletim de IA.
//!
//! Transforma o resultado de uma corrida em "beats" (pedaços de história já
//! avaliados, cada um com um peso). Em seguida filtra pelo limiar de relevância
//! e renderiza um CONTEXTO CURADO — denso em narrativa, enxuto em dados — que é
//! o que será enviado ao servidor (que escolhe o provedor — ver `client`).
//!
//! Filosofia: a inteligência de "o que é interessante" mora AQUI, não na IA.
//! A IA só redige em cima dos fatos que escolhermos (zero invenção de resultado).
//!
//! O módulo é PURO: não conhece o banco. Os beats que saem do próprio `RaceResult`
//! nascem aqui; os de CARREIRA (memória entre corridas) nascem em
//! `commands/race/noticias/`, que tem a conexão, e chegam já pesados por
//! `RaceContextInput::career_beats` — passando pelo MESMO limiar e pela MESMA
//! hierarquia. Hoje só o arco de rivalidade veio por essa porta; forma recente,
//! lesão-arco, marcos de carreira e vínculo com a equipe ainda viajam como texto
//! sem peso em `context_facts` e são os próximos candidatos a virar beat.

mod beats;
mod consulta;
mod contexto;
mod incidentes;
mod tese;

pub mod client;
pub mod em_voo;

#[cfg(test)]
mod tests;

// Só o que o resto do crate consome de fato. `consulta`, `incidentes` e `tese` são
// internos ao motor: os dois primeiros só expõem itens `pub(crate)` (o glob não
// reexportava nada) e a tese é consumida por `contexto.rs` via caminho direto.
pub use beats::*;
pub use contexto::*;
