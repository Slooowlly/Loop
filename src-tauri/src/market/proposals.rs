#![allow(dead_code)]

use serde::{Deserialize, Serialize};

use crate::models::enums::TeamRole;

pub fn is_real_career_debut_category(category_id: &str) -> bool {
    matches!(category_id, "mazda_rookie" | "toyota_rookie")
}

pub fn is_rookie_market_candidate(
    target_category: &str,
    candidate_category: &str,
    career_races: u32,
    career_seasons: u32,
) -> bool {
    if !is_real_career_debut_category(target_category) {
        return false;
    }

    if career_races == 0 && career_seasons == 0 {
        return true;
    }

    candidate_category == target_category && career_seasons <= 2
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketProposal {
    pub id: String,
    pub equipe_id: String,
    pub equipe_nome: String,
    pub piloto_id: String,
    pub piloto_nome: String,
    pub categoria: String,
    pub papel: TeamRole,
    pub salario_oferecido: f64,
    pub duracao_anos: i32,
    pub status: ProposalStatus,
    pub motivo_recusa: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProposalStatus {
    Pendente,
    Aceita,
    Recusada,
    Expirada,
}

impl ProposalStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProposalStatus::Pendente => "Pendente",
            ProposalStatus::Aceita => "Aceita",
            ProposalStatus::Recusada => "Recusada",
            ProposalStatus::Expirada => "Expirada",
        }
    }

    pub fn from_str_strict(value: &str) -> Result<Self, String> {
        match value.trim() {
            "Pendente" => Ok(Self::Pendente),
            "Aceita" => Ok(Self::Aceita),
            "Recusada" => Ok(Self::Recusada),
            "Expirada" => Ok(Self::Expirada),
            other => Err(format!("ProposalStatus inválido: '{other}'")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketReport {
    pub contracts_expired: i32,
    pub contracts_renewed: i32,
    pub new_signings: Vec<SigningInfo>,
    pub retirements_replaced: i32,
    pub rookies_placed: i32,
    pub proposals_made: i32,
    pub proposals_accepted: i32,
    pub proposals_rejected: i32,
    pub player_proposals: Vec<MarketProposal>,
    pub unresolved_vacancies: i32,
}

impl Default for MarketReport {
    fn default() -> Self {
        Self {
            contracts_expired: 0,
            contracts_renewed: 0,
            new_signings: Vec::new(),
            retirements_replaced: 0,
            rookies_placed: 0,
            proposals_made: 0,
            proposals_accepted: 0,
            proposals_rejected: 0,
            player_proposals: Vec::new(),
            unresolved_vacancies: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigningInfo {
    pub driver_id: String,
    pub driver_name: String,
    pub team_id: String,
    pub team_name: String,
    pub categoria: String,
    pub papel: String,
    pub tipo: String,
}

#[derive(Debug, Clone)]
pub struct Vacancy {
    pub team_id: String,
    pub team_name: String,
    pub categoria: String,
    pub category_tier: u8,
    pub car_performance: f64,
    pub budget: f64,
    pub cash_balance: f64,
    pub debt_balance: f64,
    pub financial_state: String,
    pub reputacao: f64,
    pub papel_necessario: TeamRole,
    pub piloto_existente_id: Option<String>,
}
