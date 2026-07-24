//! Enums do mercado e da imprensa: propostas, motivos de recusa e tipo de notícia.

use serde::{Deserialize, Serialize};

// ── Status da proposta de mercado ─────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ProposalStatus {
    Pendente,
    Aceita,
    Recusada,
    Expirada,
}

impl ProposalStatus {
    pub fn as_str(&self) -> &str {
        match self {
            ProposalStatus::Pendente => "Pendente",
            ProposalStatus::Aceita => "Aceita",
            ProposalStatus::Recusada => "Recusada",
            ProposalStatus::Expirada => "Expirada",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "Aceita" => ProposalStatus::Aceita,
            "Recusada" => ProposalStatus::Recusada,
            "Expirada" => ProposalStatus::Expirada,
            _ => ProposalStatus::Pendente,
        }
    }
}

// ── Motivo de recusa de proposta ──────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RefusalReason {
    SalarioBaixo,
    EquipeFraca,
    CategoriaErrada,
    BloqueioHierarquico,
    PreferenciaPessoal,
}

impl RefusalReason {
    pub fn as_str(&self) -> &str {
        match self {
            RefusalReason::SalarioBaixo => "SalarioBaixo",
            RefusalReason::EquipeFraca => "EquipeFraca",
            RefusalReason::CategoriaErrada => "CategoriaErrada",
            RefusalReason::BloqueioHierarquico => "BloqueioHierarquico",
            RefusalReason::PreferenciaPessoal => "PreferenciaPessoal",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "SalarioBaixo" => RefusalReason::SalarioBaixo,
            "EquipeFraca" => RefusalReason::EquipeFraca,
            "CategoriaErrada" => RefusalReason::CategoriaErrada,
            "BloqueioHierarquico" => RefusalReason::BloqueioHierarquico,
            _ => RefusalReason::PreferenciaPessoal,
        }
    }
}

// ── Tipo de notícia ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NewsType {
    Contratacao,
    Corrida,
    Lesao,
    Aposentadoria,
    Promocao,
    Rivalidade,
    Titulo,
    Incidente,
}

impl NewsType {
    pub fn as_str(&self) -> &str {
        match self {
            NewsType::Contratacao => "Contratacao",
            NewsType::Corrida => "Corrida",
            NewsType::Lesao => "Lesao",
            NewsType::Aposentadoria => "Aposentadoria",
            NewsType::Promocao => "Promocao",
            NewsType::Rivalidade => "Rivalidade",
            NewsType::Titulo => "Titulo",
            NewsType::Incidente => "Incidente",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "Contratacao" => NewsType::Contratacao,
            "Lesao" => NewsType::Lesao,
            "Aposentadoria" => NewsType::Aposentadoria,
            "Promocao" => NewsType::Promocao,
            "Rivalidade" => NewsType::Rivalidade,
            "Titulo" => NewsType::Titulo,
            "Incidente" => NewsType::Incidente,
            _ => NewsType::Corrida,
        }
    }
}
