//! Enums do piloto: status, personalidades, hierarquia na equipe e lesões.

use serde::{Deserialize, Serialize};

// ── Status do piloto ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DriverStatus {
    Ativo,
    Lesionado,
    Aposentado,
    Suspenso,
}

impl DriverStatus {
    pub fn as_str(&self) -> &str {
        match self {
            DriverStatus::Ativo => "Ativo",
            DriverStatus::Lesionado => "Lesionado",
            DriverStatus::Aposentado => "Aposentado",
            DriverStatus::Suspenso => "Suspenso",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "Lesionado" => DriverStatus::Lesionado,
            "Aposentado" => DriverStatus::Aposentado,
            "Suspenso" => DriverStatus::Suspenso,
            _ => DriverStatus::Ativo,
        }
    }

    /// Parser estrito para leitura de banco de dados.
    /// Erros de valor inválido são propagados — sem fallback silencioso.
    /// Para uso em row mappers de queries. Manter from_str() para contextos permissivos.
    pub fn from_str_strict(s: &str) -> Result<Self, String> {
        match s.trim() {
            "Ativo" => Ok(DriverStatus::Ativo),
            "Lesionado" => Ok(DriverStatus::Lesionado),
            "Aposentado" => Ok(DriverStatus::Aposentado),
            "Suspenso" => Ok(DriverStatus::Suspenso),
            other => Err(format!("DriverStatus inválido: '{other}'")),
        }
    }
}

// ── Personalidade primária ────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrimaryPersonality {
    Ambicioso,
    Consolidador,
    Mercenario,
    Leal,
}

impl PrimaryPersonality {
    pub fn as_str(&self) -> &str {
        match self {
            PrimaryPersonality::Ambicioso => "Ambicioso",
            PrimaryPersonality::Consolidador => "Consolidador",
            PrimaryPersonality::Mercenario => "Mercenario",
            PrimaryPersonality::Leal => "Leal",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "Ambicioso" => PrimaryPersonality::Ambicioso,
            "Consolidador" | "Tecnico" | "Consistente" => PrimaryPersonality::Consolidador,
            "Mercenario" | "Agressivo" => PrimaryPersonality::Mercenario,
            "Leal" | "Calmo" => PrimaryPersonality::Leal,
            _ => PrimaryPersonality::Ambicioso,
        }
    }

    pub fn from_str_strict(s: &str) -> Result<Self, String> {
        match s.trim() {
            "Ambicioso" => Ok(PrimaryPersonality::Ambicioso),
            "Consolidador" | "Tecnico" | "Consistente" => Ok(PrimaryPersonality::Consolidador),
            "Mercenario" | "Agressivo" => Ok(PrimaryPersonality::Mercenario),
            "Leal" | "Calmo" => Ok(PrimaryPersonality::Leal),
            other => Err(format!("PrimaryPersonality invalido: '{other}'")),
        }
    }
}

// ── Personalidade secundária ──────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SecondaryPersonality {
    CabecaQuente,
    SangueFrio,
    Apostador,
    Calculista,
    Showman,
    TeamPlayer,
    Solitario,
    Estudioso,
}

impl SecondaryPersonality {
    pub fn as_str(&self) -> &str {
        match self {
            SecondaryPersonality::CabecaQuente => "CabecaQuente",
            SecondaryPersonality::SangueFrio => "SangueFrio",
            SecondaryPersonality::Apostador => "Apostador",
            SecondaryPersonality::Calculista => "Calculista",
            SecondaryPersonality::Showman => "Showman",
            SecondaryPersonality::TeamPlayer => "TeamPlayer",
            SecondaryPersonality::Solitario => "Solitario",
            SecondaryPersonality::Estudioso => "Estudioso",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "CabecaQuente" => SecondaryPersonality::CabecaQuente,
            "SangueFrio" | "Sensivel" => SecondaryPersonality::SangueFrio,
            "Apostador" | "Competitivo" => SecondaryPersonality::Apostador,
            "Calculista" => SecondaryPersonality::Calculista,
            "Showman" | "Lider" => SecondaryPersonality::Showman,
            "TeamPlayer" | "Trabalhador" => SecondaryPersonality::TeamPlayer,
            "Solitario" => SecondaryPersonality::Solitario,
            "Estudioso" | "Inteligente" => SecondaryPersonality::Estudioso,
            _ => SecondaryPersonality::Calculista,
        }
    }

    pub fn from_str_strict(s: &str) -> Result<Self, String> {
        match s.trim() {
            "CabecaQuente" => Ok(SecondaryPersonality::CabecaQuente),
            "SangueFrio" | "Sensivel" => Ok(SecondaryPersonality::SangueFrio),
            "Apostador" | "Competitivo" => Ok(SecondaryPersonality::Apostador),
            "Calculista" => Ok(SecondaryPersonality::Calculista),
            "Showman" | "Lider" => Ok(SecondaryPersonality::Showman),
            "TeamPlayer" | "Trabalhador" => Ok(SecondaryPersonality::TeamPlayer),
            "Solitario" => Ok(SecondaryPersonality::Solitario),
            "Estudioso" | "Inteligente" => Ok(SecondaryPersonality::Estudioso),
            other => Err(format!("SecondaryPersonality invalido: '{other}'")),
        }
    }
}

// ── Hierarquia da equipe (N1/N2) ──────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DriverHierarchyRole {
    N1,
    N2,
    Independente,
}

impl DriverHierarchyRole {
    pub fn as_str(&self) -> &str {
        match self {
            DriverHierarchyRole::N1 => "N1",
            DriverHierarchyRole::N2 => "N2",
            DriverHierarchyRole::Independente => "Independente",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "N1" => DriverHierarchyRole::N1,
            "N2" => DriverHierarchyRole::N2,
            _ => DriverHierarchyRole::Independente,
        }
    }
}

// ── Tipo de lesão ─────────────────────────────────────────────────────────────

/// Gravidade da lesão. **Duas grafias, cada uma com o seu dono:**
///
/// - [`InjuryType::as_str`] é a grafia do BANCO ("Leve"/"Moderada"/"Grave"/"Critica"). É
///   valor de coluna gravado desde a primeira versão e não se mexe nele.
/// - [`InjuryType::chave`] — e o serde — é a grafia do FIO ("light"/"moderate"/"severe"/
///   "critical"). O frontend recebia a grafia do banco e mapeava "Leve"→"light" por conta
///   própria, incluindo o "Critica" sem acento: uma correção de acentuação no backend
///   apagava a gravidade da lesão em silêncio. A tradução agora acontece na borda.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InjuryType {
    #[serde(rename = "light")]
    Leve,
    #[serde(rename = "moderate")]
    Moderada,
    #[serde(rename = "severe")]
    Grave,
    #[serde(rename = "critical")]
    Critica,
}

impl InjuryType {
    /// A grafia do BANCO — o que está gravado na coluna `injuries.type`.
    pub fn as_str(&self) -> &'static str {
        match self {
            InjuryType::Leve => "Leve",
            InjuryType::Moderada => "Moderada",
            InjuryType::Grave => "Grave",
            InjuryType::Critica => "Critica",
        }
    }

    /// A chave do FIO — estável, sem acento e sem prosa, casada com o serde acima e com as
    /// chaves i18n que o frontend já usa (`…injurySeverity.light`, `.moderate`, …).
    pub fn chave(&self) -> &'static str {
        match self {
            InjuryType::Leve => "light",
            InjuryType::Moderada => "moderate",
            InjuryType::Grave => "severe",
            InjuryType::Critica => "critical",
        }
    }

    /// Lesão que tira o piloto de circulação de verdade (o selo 🚑 da classificação).
    pub fn e_seria(&self) -> bool {
        matches!(self, InjuryType::Grave | InjuryType::Critica)
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "Moderada" => InjuryType::Moderada,
            "Grave" => InjuryType::Grave,
            "Critica" => InjuryType::Critica,
            _ => InjuryType::Leve,
        }
    }

    /// Parser estrito para leitura de banco de dados.
    /// Erros de valor inválido são propagados — sem fallback silencioso.
    pub fn from_str_strict(s: &str) -> Result<Self, String> {
        match s.trim() {
            "Leve" => Ok(InjuryType::Leve),
            "Moderada" => Ok(InjuryType::Moderada),
            "Grave" => Ok(InjuryType::Grave),
            "Critica" => Ok(InjuryType::Critica),
            other => Err(format!("InjuryType inválido: '{other}'")),
        }
    }
}

#[cfg(test)]
mod tests_lesao {
    use super::InjuryType;

    const TODAS: [InjuryType; 4] = [
        InjuryType::Leve,
        InjuryType::Moderada,
        InjuryType::Grave,
        InjuryType::Critica,
    ];

    /// AS DUAS GRAFIAS NÃO PODEM SE MISTURAR.
    ///
    /// A do banco vai para a coluna e volta pelo parser estrito; a do fio vai para o React,
    /// que a usa como sufixo de chave i18n. Era uma só, em português, e o frontend traduzia
    /// "Leve"→"light" por conta própria — inclusive o "Critica" sem acento. Corrigir a
    /// acentuação do backend teria apagado a gravidade da lesão na tela, sem erro nenhum.
    #[test]
    fn a_grafia_do_banco_e_a_do_fio_seguem_separadas() {
        for tipo in TODAS {
            // Banco: em português, com a grafia gravada desde sempre, e o parser fecha o ciclo.
            assert_eq!(InjuryType::from_str_strict(tipo.as_str()), Ok(tipo));
            // Fio: chave estável, sem acento e sem prosa — e serde diz o MESMO que `chave()`.
            assert_eq!(
                serde_json::to_string(&tipo).unwrap(),
                format!("\"{}\"", tipo.chave())
            );
            assert!(
                tipo.chave().is_ascii() && tipo.chave() == tipo.chave().to_lowercase(),
                "chave de fio com acento ou maiúscula: {}",
                tipo.chave()
            );
        }
        assert_eq!(
            TODAS.map(|t| t.chave()),
            ["light", "moderate", "severe", "critical"]
        );
        assert_eq!(
            TODAS.map(|t| t.as_str()),
            ["Leve", "Moderada", "Grave", "Critica"]
        );
    }

    /// O corte do selo 🚑 da classificação, que o frontend guardava como um `Set` de palavras
    /// em português.
    #[test]
    fn lesao_seria_e_grave_ou_critica() {
        assert!(!InjuryType::Leve.e_seria());
        assert!(!InjuryType::Moderada.e_seria());
        assert!(InjuryType::Grave.e_seria());
        assert!(InjuryType::Critica.e_seria());
    }
}
