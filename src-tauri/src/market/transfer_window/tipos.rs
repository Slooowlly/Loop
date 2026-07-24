//! DTOs do leilão da janela: vaga, candidato, assinatura, resultado, config e a
//! oferta apresentada ao jogador.

use serde::{Deserialize, Serialize};

/// Uma vaga aberta (assento de uma equipe).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Seat {
    pub id: String,
    pub team_id: String,
    pub category: String,
    pub class: Option<String>,
    pub tier: u8,
    pub is_n1: bool,
    pub car_norm: f64,        // car_performance normalizado 0-100(+)
    pub prestige: f64,        // 0-100 (competitividade dos últimos 10 anos)
    pub required_license: u8, // licença mínima exigida pela categoria/classe
    pub salary_floor: f64,
    pub salary_ceiling: f64, // máximo que o orçamento comporta
}

/// Um piloto disponível (agente livre / "resto").
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candidate {
    pub id: String,
    pub skill: f64,
    pub tier: u8,                    // tier de skill (onde ele compete bem)
    pub brand: Option<String>,       // "mazda"/"toyota" nos tiers 0-1; None acima
    pub slam_target: Option<String>, // categoria-alvo do slam (bônus)
    pub max_license: u8,             // licença máxima do piloto (limita as categorias acessíveis)
    pub market_value: f64,
    pub ai_respects_brand: bool, // IA respeita a marca; jogador (false) tem liberdade
    #[serde(default)]
    pub category: String, // categoria de ORIGEM (onde correu por último) — p/ promovido/rebaixado
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signing {
    pub seat_id: String,
    pub team_id: String,
    pub driver_id: String,
    pub category: String,
    pub class: Option<String>,
    pub salary: f64,
    pub week: u32,
    #[serde(default)]
    pub from_category: Option<String>, // categoria de origem (None = estreia/desconhecida)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowResult {
    pub signings: Vec<Signing>,
    pub unsigned: Vec<Candidate>, // sobraram sem vaga (pós rede de segurança)
    pub weeks: u32,
}

/// Parâmetros calibrados (design §11).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowConfig {
    pub bid_gap_close: f64,    // 0.30 — fecha 30% da folga até o teto/semana
    pub shortlist_top: usize,  // 3 (tiers >= low_tier_cap+1)
    pub shortlist_low: usize,  // 2 (tiers <= low_tier_cap)
    pub low_tier_cap: u8,      // 2
    pub accept_threshold: f64, // 50.0 (semana 1)
    pub threshold_decay: f64,  // 4.0/semana — o piloto fica menos exigente
    pub threshold_floor: f64,  // 35.0 — mínimo
    pub dignity_tier_gap: u8,  // 2 — recusa cair 2+ tiers abaixo do seu nível
    pub craque_skill: f64,     // 80.0 — sempre acha vaga; pode descer (slam)
    pub hard_week_cap: u32,    // 10
    // pesos do score do piloto — TIER É RELATIVO (subir/lateral/descer)
    pub tier_base: f64,  // 12 — lateral (mesmo tier)
    pub tier_step: f64,  // 7 — por tier de diferença (sobe soma, desce subtrai)
    pub w_prestige: f64, // 22
    pub w_car: f64,      // 18
    pub w_salary: f64,   // 15
    pub w_role_n1: f64,  // 15
    pub w_role_n2: f64,  // 10
    pub slam_bonus: f64, // 18 — categoria-alvo
}

// Pesos do casamento assento↔piloto (carro vs. prestígio). FONTE ÚNICA: o desempate de vagas
// da escada viva (`market::pipeline::seat_desirability`) reusa exatamente estes pesos, então
// ajustar o leilão ajusta o desempate junto — sem duas cópias divergindo em silêncio.
pub const SEAT_W_CAR: f64 = 18.0;
pub const SEAT_W_PRESTIGE: f64 = 22.0;

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            bid_gap_close: 0.30,
            shortlist_top: 3,
            shortlist_low: 2,
            low_tier_cap: 2,
            accept_threshold: 50.0,
            threshold_decay: 4.0,
            threshold_floor: 28.0,
            dignity_tier_gap: 2,
            craque_skill: 80.0,
            hard_week_cap: 10,
            tier_base: 12.0,
            tier_step: 7.0,
            w_prestige: SEAT_W_PRESTIGE,
            w_car: SEAT_W_CAR,
            w_salary: 15.0,
            w_role_n1: 15.0,
            w_role_n2: 10.0,
            slam_bonus: 18.0,
        }
    }
}

/// Oferta apresentada ao JOGADOR (uma das que ele recebeu na semana).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerOffer {
    pub seat_id: String,
    pub team_id: String,
    pub category: String,
    pub class: Option<String>,
    pub salary: f64,
    pub is_n1: bool,
    /// Interesse ATIVO (Fase 2a do estrelato): o time cobiça o nome do jogador pelo
    /// apelo comercial da fama → oferta com salário-prêmio e destaque na UI.
    #[serde(default)]
    pub active_interest: bool,
}
