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

    /// Parser estrito para leitura de banco de dados.
    /// Erros de valor inválido são propagados — sem fallback silencioso.
    /// Para uso em row mappers de queries. É o ÚNICO parser: o `from_str()` permissivo,
    /// que devolvia `Ativo` para qualquer texto desconhecido, saiu em 12/08/2026 sem
    /// nenhum chamador.
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

    /// Lesão que tira o piloto de circulação de verdade.
    ///
    /// **Não é quem decide o selo 🚑 da classificação.** Quem decide é o
    /// `SEVERE_INJURY_TYPES` de `src/components/standings/DriverStandingsTable.jsx`, com a
    /// mesma lista de palavras em português. Esta função é o espelho Rust desse corte, e
    /// existe para o teste `lesao_seria_e_grave_ou_critica` cobrar que os dois lados
    /// concordem. Fica sem chamador de produção de propósito: apagá-la deixaria o corte
    /// escrito só no JSX.
    #[allow(dead_code)]
    pub fn e_seria(&self) -> bool {
        matches!(self, InjuryType::Grave | InjuryType::Critica)
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
