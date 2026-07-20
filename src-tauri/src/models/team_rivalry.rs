#![allow(dead_code)]
//! Rivalidade entre EQUIPES — modelo de domínio (Fase 1: fundação).
//!
//! Espelha [`crate::models::rivalry`] (piloto↔piloto) para o par de TIMES. O motor de
//! dois eixos é o MESMO — as funções puras `perceived_intensity`, `rivalry_lifecycle` e
//! `normalize_pair` de `models::rivalry` são agnósticas de piloto (operam só sobre
//! f64/strings) e por isso são **reutilizadas como estão**. O que é próprio de time é só
//! o tipo de origem ([`TeamRivalryType`]) e o carregador do par ([`TeamRivalry`]).
//!
//! Ver `docs/superpowers/specs/2026-07-19-team-rivalry-design.md`.

use serde::{Deserialize, Serialize};

use crate::models::rivalry::perceived_intensity;

// ── Tipo de origem da rivalidade de equipe ────────────────────────────────────

/// De onde a rivalidade entre dois times NASCEU (define a narrativa; preservado nos
/// reforços, igual ao `RivalryType` de piloto).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TeamRivalryType {
    /// Briga de construtores na tabela (fonte 1).
    Campeonato,
    /// Roubo de talento — um piloto saiu de um time para o outro (fonte 2, o Elo 2).
    Mercado,
    /// Carros dos dois times se batendo repetidamente na pista (fonte 3).
    Pista,
    /// Transbordamento de uma rivalidade intensa entre pilotos dos dois times (fonte 4).
    Herdada,
}

impl TeamRivalryType {
    pub fn as_str(&self) -> &str {
        match self {
            TeamRivalryType::Campeonato => "Campeonato",
            TeamRivalryType::Mercado => "Mercado",
            TeamRivalryType::Pista => "Pista",
            TeamRivalryType::Herdada => "Herdada",
        }
    }

    /// Converte da string persistida. Desconhecido → `Campeonato` (fallback conservador;
    /// a camada de query rejeita tipos inválidos de forma explícita — ver
    /// `db::queries::team_rivalries`).
    pub fn from_str(s: &str) -> Self {
        match s {
            "Mercado" => TeamRivalryType::Mercado,
            "Pista" => TeamRivalryType::Pista,
            "Herdada" => TeamRivalryType::Herdada,
            _ => TeamRivalryType::Campeonato,
        }
    }
}

// ── Model de domínio ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamRivalry {
    pub id: String,
    /// ID sempre ordenado: `team1_id < team2_id` (string).
    pub team1_id: String,
    pub team2_id: String,
    /// Peso acumulado ao longo da história — decai lentamente entre temporadas (0.0–100.0).
    pub historical_intensity: f64,
    /// Calor recente — aquece rápido, esfria com mais força entre temporadas (0.0–100.0).
    pub recent_activity: f64,
    pub tipo: TeamRivalryType,
    pub criado_em: String,
    pub ultima_atualizacao: String,
    /// Número da temporada do último reforço — usado para decidir o decaimento.
    pub temporada_update: i32,
}

impl TeamRivalry {
    /// Intensidade percebida: `0.4·historical + 0.6·recent` (idêntica à de piloto).
    pub fn perceived_intensity(&self) -> f64 {
        perceived_intensity(self.historical_intensity, self.recent_activity)
    }
}

// ── Testes ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn team_rivalry_type_roundtrip() {
        for t in [
            TeamRivalryType::Campeonato,
            TeamRivalryType::Mercado,
            TeamRivalryType::Pista,
            TeamRivalryType::Herdada,
        ] {
            assert_eq!(TeamRivalryType::from_str(t.as_str()), t);
        }
    }

    #[test]
    fn from_str_desconhecido_cai_em_campeonato() {
        assert_eq!(TeamRivalryType::from_str("qualquer"), TeamRivalryType::Campeonato);
    }

    #[test]
    fn perceived_usa_mesma_formula_do_piloto() {
        let r = TeamRivalry {
            id: "TRV001".to_string(),
            team1_id: "T001".to_string(),
            team2_id: "T002".to_string(),
            historical_intensity: 10.0,
            recent_activity: 20.0,
            tipo: TeamRivalryType::Campeonato,
            criado_em: String::new(),
            ultima_atualizacao: String::new(),
            temporada_update: 1,
        };
        // 0.4*10 + 0.6*20 = 16.0
        assert!((r.perceived_intensity() - 16.0).abs() < 1e-9);
    }
}
