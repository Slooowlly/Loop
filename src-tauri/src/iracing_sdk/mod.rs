//! Módulo isolado de TESTE do SDK do iRacing.
//!
//! Diferente do resto do app (um simulador de carreira *offline*), este módulo
//! fala com o iRacing REAL rodando na máquina. O SDK do iRacing publica seus
//! dados num arquivo mapeado em memória (`Local\IRSDKMemMapFileName`); aqui
//! abrimos esse mapeamento "na mão" (sem crate intermediária) e lemos o bloco de
//! informação de sessão, que é uma string YAML com pista, carros, pilotos e
//! classes.
//!
//! Escopo deliberadamente pequeno: conectar, validar e devolver a string de
//! sessão (+ alguns campos do cabeçalho). Telemetria por tick fica de fora.
//!
//! O binding real só existe em Windows; em outros alvos há um stub que devolve
//! [`IracingError::Unsupported`], para a lib continuar compilando em qualquer SO.

mod api;
mod constantes;
mod custid;
mod imp;
mod tipos;
mod yaml;

pub mod adaptive;
pub mod aiseason_results;
pub mod behavior;
pub mod car_difficulty;
pub mod paint_gen;
pub mod paths;
pub mod race_capture;
pub mod race_control;
pub mod race_monitor;
pub mod result_bridge;
pub mod results_gen;
pub mod rivalry_perception;
pub mod session_results;
pub mod roster_gen;
pub mod season_gen;
pub mod telemetry_analysis;
pub mod tire_strategy;
pub mod weather;

pub use api::*;
pub(crate) use constantes::*;
pub use custid::*;
pub use tipos::*;
pub use yaml::*;
