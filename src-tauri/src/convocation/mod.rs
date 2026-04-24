pub mod eligibility;
pub mod pipeline;
pub mod player_offers;
pub mod quotas;
pub mod scoring;
pub mod special_window;

pub use pipeline::{
    advance_to_convocation_window, encerrar_bloco_especial, iniciar_bloco_especial,
    run_convocation_window, run_pos_especial, ConvocationResult, PosEspecialResult,
};
pub use player_offers::PlayerSpecialOffer;
