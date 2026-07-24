//! DTOs do mercado de pilotos expostos à UI (prévia de agentes livres).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreeAgentPreview {
    pub driver_id: String,
    pub driver_name: String,
    pub categoria: String,
    pub is_rookie: bool,
    pub previous_team_name: Option<String>,
    pub previous_team_color: Option<String>,
    pub previous_team_abbr: Option<String>,
    pub seasons_at_last_team: i32,
    pub total_career_seasons: i32,
    pub license_nivel: String,
    pub license_sigla: String,
    pub last_championship_position: Option<i32>,
    pub last_championship_total_drivers: Option<i32>,
    /// Tier de prestígio (0=Rookie … 6=Endurance) da categoria onde ele corre hoje.
    /// É a chave de agrupamento da coluna (faixa de nível). `None` = rookie/sem categoria.
    pub market_tier: Option<u8>,
    /// Temporadas parado (ver `FreeAgentRaw::seasons_idle`). Usado pelo marcador "parado".
    pub seasons_idle: Option<i32>,
    /// IDs das categorias onde ele pode realmente pegar vaga (mesma regra do leilão:
    /// tier ±1 + licença exigida, com +1 de promoção liberado). Usado pelo filtro do topo.
    pub eligible_categories: Vec<String>,
}
