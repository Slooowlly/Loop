//! Enums da temporada: status e fase (modelo 9D + legado).
//!
//! Dificuldade NÃO mora aqui. Havia um `enum Difficulty` neste arquivo que nunca teve
//! consumidor: o crate inteiro trafega dificuldade como texto, normalizado em
//! [`crate::models::driver_generation`], que é quem aceita as grafias com e sem acento e
//! o alias `Elite`. O enum saiu em 12/08/2026.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SeasonStatus {
    EmAndamento,
    Finalizada,
}

impl SeasonStatus {
    pub fn as_str(&self) -> &str {
        match self {
            SeasonStatus::EmAndamento => "EmAndamento",
            SeasonStatus::Finalizada => "Finalizada",
        }
    }

    /// Parser estrito para leitura de banco de dados.
    /// Erros de valor inválido são propagados — sem fallback silencioso.
    /// Preserva alias legacy "Ativa" → EmAndamento.
    pub fn from_str_strict(s: &str) -> Result<Self, String> {
        match s.trim() {
            "EmAndamento" | "Ativa" => Ok(SeasonStatus::EmAndamento),
            "Finalizada" => Ok(SeasonStatus::Finalizada),
            other => Err(format!("SeasonStatus inválido: '{other}'")),
        }
    }
}

impl std::fmt::Display for SeasonStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ── Fase da temporada ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SeasonPhase {
    // ── Modelo 9D ────────────────────────────────────────────────────────────
    /// Mercado de pré-temporada (dezembro–fevereiro). Contratações, builds de carro.
    PreTemporada,
    /// Bloco de corridas (fevereiro–novembro). Geração de calendário e simulação ativas.
    Temporada,
    /// Apuração final antes de advance_season: standings, prêmios, expirações.
    Encerramento,

    // ── Legado ────────────────────────────────────────────────────────────────
    /// LEGADO 9D: apenas para temporadas em voo pré-v33; remover na fase pós-9D.
    BlocoRegular,
    /// LEGADO 9D: apenas para temporadas em voo pré-v33; remover na fase pós-9D.
    JanelaConvocacao,
    /// LEGADO 9D: apenas para temporadas em voo pré-v33; remover na fase pós-9D.
    BlocoEspecial,
    /// LEGADO 9D: apenas para temporadas em voo pré-v33; remover na fase pós-9D.
    /// Fase de encerramento após o bloco especial: desmontagem administrativa
    /// (expiração de contratos especiais, limpeza de lineups) e repercussões.
    PosEspecial,
}

impl SeasonPhase {
    pub fn as_str(&self) -> &str {
        match self {
            SeasonPhase::PreTemporada => "PreTemporada",
            SeasonPhase::Temporada => "Temporada",
            SeasonPhase::Encerramento => "Encerramento",
            SeasonPhase::BlocoRegular => "BlocoRegular",
            SeasonPhase::JanelaConvocacao => "JanelaConvocacao",
            SeasonPhase::BlocoEspecial => "BlocoEspecial",
            SeasonPhase::PosEspecial => "PosEspecial",
        }
    }

    /// Parser estrito para leitura de banco de dados.
    pub fn from_str_strict(s: &str) -> Result<Self, String> {
        match s.trim() {
            "PreTemporada" => Ok(SeasonPhase::PreTemporada),
            "Temporada" => Ok(SeasonPhase::Temporada),
            "Encerramento" => Ok(SeasonPhase::Encerramento),
            "BlocoRegular" => Ok(SeasonPhase::BlocoRegular),
            "JanelaConvocacao" => Ok(SeasonPhase::JanelaConvocacao),
            "BlocoEspecial" => Ok(SeasonPhase::BlocoEspecial),
            "PosEspecial" => Ok(SeasonPhase::PosEspecial),
            other => Err(format!("SeasonPhase inválido: '{other}'")),
        }
    }

    /// True para as quatro variantes do modelo pré-9D.
    pub fn is_legacy(&self) -> bool {
        matches!(
            self,
            SeasonPhase::BlocoRegular
                | SeasonPhase::JanelaConvocacao
                | SeasonPhase::BlocoEspecial
                | SeasonPhase::PosEspecial
        )
    }

    /// True quando a temporada está em fase de corridas ativas.
    pub fn is_racing(&self) -> bool {
        matches!(
            self,
            SeasonPhase::Temporada | SeasonPhase::BlocoRegular | SeasonPhase::BlocoEspecial
        )
    }
}

impl std::fmt::Display for SeasonPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::SeasonPhase;

    const ALL_PHASES: &[(SeasonPhase, &str)] = &[
        (SeasonPhase::PreTemporada, "PreTemporada"),
        (SeasonPhase::Temporada, "Temporada"),
        (SeasonPhase::Encerramento, "Encerramento"),
        (SeasonPhase::BlocoRegular, "BlocoRegular"),
        (SeasonPhase::JanelaConvocacao, "JanelaConvocacao"),
        (SeasonPhase::BlocoEspecial, "BlocoEspecial"),
        (SeasonPhase::PosEspecial, "PosEspecial"),
    ];

    #[test]
    fn season_phase_roundtrip() {
        for (phase, s) in ALL_PHASES {
            assert_eq!(phase.as_str(), *s, "as_str falhou para {s}");
            assert_eq!(
                SeasonPhase::from_str_strict(s).unwrap(),
                *phase,
                "from_str_strict falhou para {s}"
            );
            assert_eq!(phase.to_string(), *s, "Display falhou para {s}");
        }
    }

    #[test]
    fn season_phase_from_str_strict_rejects_unknown() {
        assert!(SeasonPhase::from_str_strict("Desconhecido").is_err());
        assert!(SeasonPhase::from_str_strict("").is_err());
        assert!(
            SeasonPhase::from_str_strict(" Temporada ").is_ok(),
            "trim deve funcionar"
        );
    }

    #[test]
    fn season_phase_is_legacy() {
        assert!(!SeasonPhase::PreTemporada.is_legacy());
        assert!(!SeasonPhase::Temporada.is_legacy());
        assert!(!SeasonPhase::Encerramento.is_legacy());
        assert!(SeasonPhase::BlocoRegular.is_legacy());
        assert!(SeasonPhase::JanelaConvocacao.is_legacy());
        assert!(SeasonPhase::BlocoEspecial.is_legacy());
        assert!(SeasonPhase::PosEspecial.is_legacy());
    }

    #[test]
    fn season_phase_is_racing() {
        assert!(!SeasonPhase::PreTemporada.is_racing());
        assert!(SeasonPhase::Temporada.is_racing());
        assert!(!SeasonPhase::Encerramento.is_racing());
        assert!(SeasonPhase::BlocoRegular.is_racing());
        assert!(!SeasonPhase::JanelaConvocacao.is_racing());
        assert!(SeasonPhase::BlocoEspecial.is_racing());
        assert!(!SeasonPhase::PosEspecial.is_racing());
    }
}
