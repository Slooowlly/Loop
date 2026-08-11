//! Suíte de testes do cérebro de manutenção do carro (extraída de
//! `car_maintenance.rs`).
//!
//! Continua sendo o mesmo módulo `tests` de antes: cada fatia faz
//! `use super::super::*` e enxerga o módulo `car_maintenance` inteiro, incluindo os
//! itens privados. Aqui ficam só as declarações — os casos moram nas fatias por tema.

/// Desgaste, contato de disputa, quebra e a conta do enduro.
mod desgaste;
/// Testes de diagnóstico (`#[ignore]`) que imprimem o comportamento medido.
mod diagnostico;
/// Horizonte de planejamento e DNA da equipe.
mod horizonte_e_dna;
/// O spread da grade: só o orçamento separando os carros.
mod spread;
/// A escolha do upgrade e a cota da janela.
mod upgrades;
