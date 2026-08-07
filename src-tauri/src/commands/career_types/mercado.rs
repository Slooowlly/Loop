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

/// O que o JOGADOR já viveu com um piloto que ficou sem vaga.
///
/// A lista de deslocados sozinha é uma lista de estranhos: seis nomes que o
/// jogador nunca ouviu falar, e no meio deles um que ele bateu na última volta
/// de Interlagos. Este DTO é o que separa os dois casos.
///
/// Só vem preenchido para quem realmente dividiu grid com o jogador — quem nunca
/// cruzou com ele fica com tudo em zero, e a UI não desenha nada.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DisplacedDriverContext {
    pub driver_id: String,
    /// Corridas em que os dois têm resultado: dividiram o grid.
    pub shared_races: i32,
    /// Dessas, quantas o jogador terminou à frente. Abandono dos dois lados fica
    /// de fora do placar — quebrar o motor não é perder um duelo.
    pub player_ahead: i32,
    pub driver_ahead: i32,
    /// Número da última temporada em que se encontraram.
    pub last_shared_season: Option<i32>,
    /// `"nemesis"` | `"rival"` | `None`, com o MESMO critério das outras telas —
    /// sai de `select_player_interests`, não de um limiar próprio. Duas definições
    /// de "quem é rival" no mesmo jogo divergem na primeira vez que uma muda.
    pub rival_role: Option<String>,
}
