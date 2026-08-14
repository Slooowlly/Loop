//! Enums de contrato: status, papel do piloto na equipe e tipo de vínculo.

use serde::{Deserialize, Serialize};

// ── Status do contrato ────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContractStatus {
    Ativo,
    Expirado,
    Rescindido,
    Pendente,
}

impl ContractStatus {
    pub fn as_str(&self) -> &str {
        match self {
            ContractStatus::Ativo => "Ativo",
            ContractStatus::Expirado => "Expirado",
            ContractStatus::Rescindido => "Rescindido",
            ContractStatus::Pendente => "Pendente",
        }
    }
}

impl std::fmt::Display for ContractStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ── Papel do piloto na equipe ─────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TeamRole {
    Numero1,
    Numero2,
}

impl TeamRole {
    pub fn as_str(&self) -> &str {
        match self {
            TeamRole::Numero1 => "Numero1",
            TeamRole::Numero2 => "Numero2",
        }
    }

    /// Parser estrito para leitura de banco de dados. Aceita os aliases legados
    /// (`N1`/`Titular`, `N2`/`Reserva`/`Junior`) porque eles estão gravados em saves
    /// antigos; qualquer outro valor vira erro, sem fallback silencioso.
    pub fn from_str_strict(s: &str) -> Result<Self, String> {
        match s.trim() {
            "Numero1" | "N1" | "Titular" => Ok(TeamRole::Numero1),
            "Numero2" | "N2" | "Reserva" | "Junior" => Ok(TeamRole::Numero2),
            other => Err(format!("TeamRole inválido: '{other}'")),
        }
    }
}

impl std::fmt::Display for TeamRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ── Tipo de contrato ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContractType {
    Regular,
    Especial,
}

impl ContractType {
    pub fn as_str(&self) -> &str {
        match self {
            ContractType::Regular => "Regular",
            ContractType::Especial => "Especial",
        }
    }

    /// Parser estrito para leitura de banco de dados.
    /// Erros de valor inválido são propagados — sem fallback silencioso.
    /// Para criação interna, use ContractType::Regular diretamente.
    pub fn from_str_strict(s: &str) -> Result<Self, String> {
        match s.trim() {
            "Regular" => Ok(ContractType::Regular),
            "Especial" => Ok(ContractType::Especial),
            other => Err(format!("ContractType inválido: '{other}'")),
        }
    }
}

impl std::fmt::Display for ContractType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
