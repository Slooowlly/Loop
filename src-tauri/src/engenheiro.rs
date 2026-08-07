//! O ENGENHEIRO: o que ele entende da pergunta e o que ele sabe da corrida.
//!
//! Este módulo é a metade pensante do push-to-talk. A outra metade — ouvir, redigir e
//! falar — mora fora do app: o áudio vai para o nosso servidor, que tem as chaves do
//! Scribe, do Gemini e da Cloud TTS. Aqui não se fala com provedor nenhum.
//!
//! A divisão de trabalho é a regra da casa e vale a pena escrever por extenso, porque é
//! ela que decide a qualidade da resposta:
//!
//! - **O app calcula.** Gap, tendência, autonomia, voltas restantes — tudo chega ao
//!   modelo já resolvido, vindo do [`EstadoAgora`](crate::iracing_sdk::race_monitor::EstadoAgora).
//! - **O modelo redige.** Ele recebe fatos em português e devolve uma frase de rádio.
//!
//! Um modelo que calcula erra de um jeito particularmente ruim: erra com confiança, em
//! números, e ninguém no meio de uma corrida tem como conferir. Um modelo que só redige
//! erra no máximo o tom.
//!
//! ```text
//! transcrição ──► intencao::classificar ──► Intencao
//!                                              │
//! EstadoAgora ─────────────────────────────────┴──► fatos::dossie ──► linhas em PT
//! ```

pub mod campeonato;
pub mod classificacao;
pub mod fala;
pub mod fatos;
pub mod intencao;
pub mod marco;
pub mod memoria;
pub mod momento;
pub mod nomes;
pub mod peca_propria;
pub mod quebra;
pub mod responder;
pub mod ritmo;
pub mod tempo_volta;
pub mod tratamento;
pub mod vizinhanca;
pub mod volta_referencia;

// `renderizar` e `dossie` NÃO são reexportados aqui de propósito, desde que o campeonato
// entrou. Os dois de `fala`/`fatos` só conhecem telemetria, e chamá-los pelo nome curto era
// o caminho fácil para esquecer a tabela sem nada indicar. Quem responde de verdade é
// [`responder`], que compõe as duas fontes; os originais continuam acessíveis pelo caminho
// completo, para quem quiser exatamente a parte de telemetria.
pub use fala::catalogo;
pub use fatos::dossie_completo;
pub use intencao::{classificar, Intencao};

/// Arreio de MEDIÇÃO do rádio inteiro numa linha do tempo só. Não é asserção — despeja o JSON
/// que `scripts/analise-radio.mjs` junta com o spotter de captura real.
#[cfg(test)]
mod medicao_radio;
#[cfg(test)]
mod tests;
