//! Geracao das noticias do pos-corrida: importancia da pauta, boletim da corrida do jogador, pre-aquecimento do texto por IA e as notas das demais categorias.
//!
//! Fachada: o corpo vive nos submodulos irmaos em `noticias/`.
//!
//! | Submodulo | Papel |
//! |---|---|
//! | [`importancia`] | peso editorial da pauta |
//! | [`manchetes`] | monta e grava os itens de noticia (vencedor, campeao, incidentes, lesoes) |
//! | [`fatos_boletim`] | curadoria dos fatos narrativos que alimentam o boletim de IA |
//! | [`recordes`] | recordes e marcas historicas viram fato narrativo |
//! | [`marcos`] | gravacao dos marcos (milestones) batidos na corrida |
//! | [`campeonato`] | duelo interno e quadro do campeonato |
//! | [`persistencia`] | orquestracao, gravacao dos fatos e pre-aquecimento do texto |

#[path = "noticias/campeonato.rs"]
mod campeonato;
#[path = "noticias/fatos_boletim.rs"]
mod fatos_boletim;
#[path = "noticias/importancia.rs"]
mod importancia;
#[path = "noticias/manchetes.rs"]
mod manchetes;
#[path = "noticias/marcos.rs"]
mod marcos;
#[path = "noticias/persistencia.rs"]
mod persistencia;
#[path = "noticias/recordes.rs"]
mod recordes;

pub(super) use importancia::*;
pub(super) use persistencia::*;
