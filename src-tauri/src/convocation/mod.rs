//! Convocação e bloco especial — **fase LEGADA**, viva só para saves pré-v33.
//!
//! Conferido em 11/08/2026, porta por porta: toda entrada deste módulo passa por
//! [`pipeline::advance_to_convocation_window`], que exige
//! `season.fase == SeasonPhase::BlocoRegular`. E nenhuma carreira nova chega nessa fase:
//!
//! - a primeira temporada é criada e imediatamente carimbada `Temporada`
//!   (`commands::career::lifecycle`);
//! - toda temporada seguinte nasce `PreTemporada` e vira `Temporada` ao fechar a janela
//!   (`commands::career::market_window`);
//! - o draft histórico só chama o bloco especial quando `fase.is_legacy()` é verdadeiro
//!   (`commands::historical_draft`), que é o mesmo portão.
//!
//! O `Season::new` ainda tem `BlocoRegular` como valor de campo, mas todos os chamadores
//! sobrescrevem antes de persistir — ele não cria carreira nenhuma nessa fase.
//!
//! **Por que ainda está aqui**: um save antigo parado em `BlocoEspecial` fica sem saída se
//! o módulo sumir — não há como avançar a temporada dele. Aposentar isto é decisão de
//! produto ("paramos de abrir saves pré-v33"), e a remoção atravessa a lista de comandos
//! em `lib.rs`, `commands/convocation.rs` e as ações do `seasonSlice` no front. Enquanto
//! essa decisão não vem, o teste `fases_legadas_sao_exatamente_as_da_convocacao` (em
//! `pipeline/tests`) fixa a fronteira: se uma fase do modelo novo virar legada, ou uma
//! legada deixar de ser, ele acende.

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
