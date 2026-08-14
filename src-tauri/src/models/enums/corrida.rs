//! Enums da corrida: status, tipo de pista, incidentes, segmentos e slot temático.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrackType {
    Road,
    Roval,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RaceStatus {
    Pendente,
    Concluida,
}

impl RaceStatus {
    pub fn as_str(&self) -> &str {
        match self {
            RaceStatus::Pendente => "Pendente",
            RaceStatus::Concluida => "Concluida",
        }
    }

    /// Parser estrito para leitura de banco de dados.
    /// Erros de valor inválido são propagados — sem fallback silencioso.
    pub fn from_str_strict(s: &str) -> Result<Self, String> {
        match s.trim() {
            "Pendente" => Ok(RaceStatus::Pendente),
            "Concluida" => Ok(RaceStatus::Concluida),
            other => Err(format!("RaceStatus inválido: '{other}'")),
        }
    }
}

impl std::fmt::Display for RaceStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ── Slot temático da corrida ──────────────────────────────────────────────────

/// Papel narrativo fixo de uma corrida dentro da sua temporada.
/// Determinado no momento da geração do calendário — imutável após persistência.
///
/// Semântica: representa a intenção curatorial do calendário, não o resultado
/// da corrida, nem a importância calculada do campeonato naquele momento.
///
/// `NaoClassificado` é um valor de domínio explícito (não Option) usado para:
///   - Saves gerados antes da migration v12
///   - Corridas geradas pelo caminho legado (`select_tracks`)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThematicSlot {
    // Grupo BlocoRegular
    /// Rodada 1 de qualquer categoria regular — papel de abertura, não necessariamente
    /// abertura "prestigiosa". A diferença de prestígio entre categorias vem de outros
    /// eixos (category base score, EventInterest).
    AberturaDaTemporada,
    /// Miolo sem distinção narrativa especial.
    RodadaRegular,
    /// Rodada com pista visitante de outra região (categorias Amador/BMW com visitor_id).
    VisitanteRegional,
    /// Âncora de miolo com pista strong — usada em Endurance quando nenhum slot
    /// narrativo recebeu pista strong.
    MidpointPrestigio,
    /// Penúltima rodada com strong_penult (GT3).
    TensaoPreFinal,
    /// Última rodada em BlocoRegular.
    FinalDaTemporada,

    // Grupo BlocoEspecial
    /// Rodada 1 do bloco especial.
    AberturaEspecial,
    /// Miolo do bloco especial — sem distinção narrativa.
    RodadaEspecial,
    /// Última rodada em BlocoEspecial.
    FinalEspecial,

    /// Fallback explícito — nunca NULL no domínio Rust.
    /// NULL no banco → NaoClassificado na leitura.
    NaoClassificado,
}

impl ThematicSlot {
    pub fn as_str(&self) -> &str {
        match self {
            ThematicSlot::AberturaDaTemporada => "AberturaDaTemporada",
            ThematicSlot::RodadaRegular => "RodadaRegular",
            ThematicSlot::VisitanteRegional => "VisitanteRegional",
            ThematicSlot::MidpointPrestigio => "MidpointPrestigio",
            ThematicSlot::TensaoPreFinal => "TensaoPreFinal",
            ThematicSlot::FinalDaTemporada => "FinalDaTemporada",
            ThematicSlot::AberturaEspecial => "AberturaEspecial",
            ThematicSlot::RodadaEspecial => "RodadaEspecial",
            ThematicSlot::FinalEspecial => "FinalEspecial",
            ThematicSlot::NaoClassificado => "NaoClassificado",
        }
    }

    /// Parser estrito para leitura de banco de dados.
    ///
    /// Contrato de parse:
    ///   - NULL no banco → usar `ThematicSlot::NaoClassificado` (o chamador trata o None)
    ///   - string presente e válida → Ok(enum)
    ///   - string presente e inválida → Err (NÃO usar unwrap_or para string presente)
    pub fn from_str_strict(s: &str) -> Result<Self, String> {
        match s.trim() {
            "AberturaDaTemporada" => Ok(ThematicSlot::AberturaDaTemporada),
            "RodadaRegular" => Ok(ThematicSlot::RodadaRegular),
            "VisitanteRegional" => Ok(ThematicSlot::VisitanteRegional),
            "MidpointPrestigio" => Ok(ThematicSlot::MidpointPrestigio),
            "TensaoPreFinal" => Ok(ThematicSlot::TensaoPreFinal),
            "FinalDaTemporada" => Ok(ThematicSlot::FinalDaTemporada),
            "AberturaEspecial" => Ok(ThematicSlot::AberturaEspecial),
            "RodadaEspecial" => Ok(ThematicSlot::RodadaEspecial),
            "FinalEspecial" => Ok(ThematicSlot::FinalEspecial),
            "NaoClassificado" => Ok(ThematicSlot::NaoClassificado),
            other => Err(format!("ThematicSlot inválido: '{other}'")),
        }
    }
}

impl std::fmt::Display for ThematicSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
