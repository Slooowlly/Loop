//! Entidade de calendário (extraída de `calendar/mod.rs`).

use serde::{Deserialize, Serialize};

use crate::models::enums::{RaceStatus, SeasonPhase, ThematicSlot, WeatherCondition};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalendarEntry {
    pub id: String,
    pub season_id: String,
    pub categoria: String,
    pub rodada: i32,
    pub nome: String,
    pub track_id: u32,
    pub track_name: String,
    pub track_config: String,
    pub clima: WeatherCondition,
    pub temperatura: f64,
    pub voltas: i32,
    pub duracao_corrida_min: i32,
    pub duracao_classificacao_min: i32,
    pub status: RaceStatus,
    pub horario: String,
    /// Semana do ano (1–52) — unidade temporal interna do sistema.
    /// A ordenação e toda lógica temporal baseiam-se neste campo.
    pub week_of_year: i32,
    /// Fase da temporada em que o evento ocorre (BlocoRegular ou BlocoEspecial).
    pub season_phase: SeasonPhase,
    /// Data visual derivada de week_of_year — para UI, notícias e narrativa.
    /// Não é a base lógica do sistema; use season_week para ordenação 9D.
    pub display_date: String,
    /// Papel narrativo fixo desta corrida dentro da temporada.
    /// Determinado no momento da geração — imutável após persistência.
    /// `NaoClassificado` para saves pré-v12 ou caminho legado.
    pub thematic_slot: ThematicSlot,
    /// Posição monotônica na régua 9D (1–51). None para saves pré-v33.
    /// Adicionado à coluna DB na migração v33 (Etapa 3).
    #[serde(default)]
    pub season_week: Option<u32>,
}
