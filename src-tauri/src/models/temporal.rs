use serde::{Deserialize, Serialize};

use crate::calendar::CalendarEntry;
use crate::models::enums::SeasonPhase;

/// Estado temporal derivado da temporada ativa.
/// Combina macroestado (season.fase) com estado factual (calendar).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeasonTemporalSummary {
    pub fase: SeasonPhase,
    /// MAX(week_of_year) das corridas concluídas — a semana efetivamente alcançada.
    /// None se nenhuma corrida foi concluída ainda.
    pub effective_week: Option<i32>,
    /// Data atual visível da carreira para a UI.
    pub current_display_date: String,
    /// Próxima corrida pendente da categoria do jogador.
    /// NOTA: acoplamento temporário com CalendarEntry. Em iteração futura,
    /// pode virar um DTO temporal mais enxuto sem todos os campos de corrida.
    pub next_player_event: Option<CalendarEntry>,
    /// Data da próxima corrida do jogador, pronta para a UI.
    pub next_event_display_date: Option<String>,
    /// Distância em dias até o próximo evento.
    pub days_until_next_event: Option<i32>,
    /// Corridas pendentes na fase atual da temporada.
    pub pending_in_phase: i32,
}
