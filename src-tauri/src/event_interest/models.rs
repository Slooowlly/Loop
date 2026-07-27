// O allow cobre HOJE só `EventInterestContext::{week_of_year, track_id, track_name}`:
// insumos que o cálculo ainda não pondera mas que descrevem o evento e são preenchidos
// por todos os call sites. O restante do módulo já é lido (o `RealizedEventInterest`
// passou a sair na UI via `EventRepercussionSummary`). Ao ligar esses três campos,
// remova o allow em vez de ampliá-lo.
#![allow(dead_code)]

use serde::{Deserialize, Serialize};

use crate::models::enums::{SeasonPhase, ThematicSlot};

// ── Tier qualitativo de interesse ─────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InterestTier {
    Baixo,
    Moderado,
    Alto,
    MuitoAlto,
    EventoPrincipal,
}

// ── Insumos do cálculo pré-corrida ────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct EventInterestContext {
    pub categoria: String,
    pub season_phase: SeasonPhase,
    pub rodada: i32,
    pub total_rodadas: i32,
    pub week_of_year: i32,
    pub track_id: i32,
    pub track_name: String,

    pub is_player_event: bool,
    pub player_championship_position: Option<i32>,
    pub player_media: Option<f32>,

    pub championship_gap_to_leader: Option<i32>,
    pub is_title_decider_candidate: bool,
    /// Papel narrativo da corrida — lido diretamente do CalendarEntry.
    /// `NaoClassificado` para saves pré-v12 ou contextos sem CalendarEntry.
    pub thematic_slot: ThematicSlot,
}

// ── Resultado do cálculo ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExpectedEventInterest {
    /// Score interno contínuo (base para todos os derivados)
    pub score: f32,
    /// Valor arredondado para exibição na UI
    pub display_value: i32,
    /// Leitura qualitativa do nível de interesse
    pub tier: InterestTier,
    /// Multiplicador de pressão — uso futuro (narrativa/forma)
    pub pressure_modifier: f32,
    /// Multiplicador de mídia — uso futuro (repercussão pós-corrida)
    pub media_multiplier: f32,
    /// Multiplicador de motivação — uso futuro (efeito no piloto)
    pub motivation_multiplier: f32,
}

// ── Resultado da repercussão pós-corrida ─────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HeadlineStrength {
    Normal,
    Forte,
    Principal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealizedEventInterest {
    pub expected_display_value: i32,
    pub expected_tier: InterestTier,
    pub final_score: f32,
    pub final_display_value: i32,
    pub final_tier: InterestTier,
    pub delta_vs_expected: f32,
    pub media_delta_modifier: f32,
    pub motivation_delta_modifier: f32,
    pub news_importance_bias: i32,
    pub headline_strength: HeadlineStrength,
}

// ── DTO público para payload de carreira ─────────────────────────────────────

/// Resumo de interesse exposto ao frontend via payload de próxima corrida.
/// Contém apenas o que tem uso real na UI neste bloco.
/// pressure_modifier permanece em ExpectedEventInterest (uso interno).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventInterestSummary {
    pub display_value: i32,
    pub tier: InterestTier,
    pub tier_label: String,
}

/// Resumo da REPERCUSSÃO realizada, exposto ao frontend no payload de resultado de
/// corrida. Mesmo critério do `EventInterestSummary`: só o que tem uso real na tela.
/// Os modificadores (`media_delta_modifier`, `motivation_delta_modifier`) e o
/// `news_importance_bias` cru ficam em `RealizedEventInterest` — o jogador lê o
/// EFEITO deles pelo `headline_strength_label`, não o coeficiente.
///
/// Os `*_label` já vêm traduzidos pelo `rust-i18n` (locale global do processo): o
/// front exibe direto, sem mapa paralelo. Para LÓGICA, use os enums `final_tier` /
/// `headline_strength`, nunca o texto do label.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRepercussionSummary {
    /// O que se esperava do evento antes da largada (mesma escala do
    /// `EventInterestSummary.display_value` mostrado na Sala de Estratégia).
    pub expected_display_value: i32,
    pub expected_tier: InterestTier,
    pub expected_tier_label: String,
    /// O que o evento de fato entregou, já com resultado do jogador e contexto de título.
    pub final_display_value: i32,
    pub final_tier: InterestTier,
    pub final_tier_label: String,
    /// `final_display_value - expected_display_value`. Positivo = a corrida entregou
    /// mais do que prometia. É a leitura que a tela de resultado destaca.
    pub delta_display_value: i32,
    pub headline_strength: HeadlineStrength,
    pub headline_strength_label: String,
}
