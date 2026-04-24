#![allow(dead_code)]

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

    pub fn from_str(s: &str) -> Self {
        match s.trim() {
            "Expirado" => ContractStatus::Expirado,
            "Rescindido" => ContractStatus::Rescindido,
            "Pendente" => ContractStatus::Pendente,
            _ => ContractStatus::Ativo,
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

    pub fn from_str(s: &str) -> Self {
        match s.trim() {
            "Numero1" | "N1" | "Titular" => TeamRole::Numero1,
            "Numero2" | "N2" | "Reserva" | "Junior" => TeamRole::Numero2,
            _ => TeamRole::Numero2,
        }
    }

    /// Parser estrito para leitura de banco de dados.
    /// Preserva aliases legacy aceitos pelo parser permissivo.
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

// ── Condição climática ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RainGroup {
    Dry,
    Normal,
    Rainy,
}

impl RainGroup {
    pub fn as_str(&self) -> &str {
        match self {
            RainGroup::Dry => "Dry",
            RainGroup::Normal => "Normal",
            RainGroup::Rainy => "Rainy",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "Dry" => RainGroup::Dry,
            "Rainy" => RainGroup::Rainy,
            _ => RainGroup::Normal,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WeatherCondition {
    Dry,
    Damp,
    Wet,
    HeavyRain,
}

impl WeatherCondition {
    pub fn as_str(&self) -> &str {
        match self {
            WeatherCondition::Dry => "Dry",
            WeatherCondition::Damp => "Damp",
            WeatherCondition::Wet => "Wet",
            WeatherCondition::HeavyRain => "HeavyRain",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "Damp" => WeatherCondition::Damp,
            "Wet" => WeatherCondition::Wet,
            "HeavyRain" => WeatherCondition::HeavyRain,
            _ => WeatherCondition::Dry,
        }
    }

    pub fn from_str_strict(s: &str) -> Result<Self, String> {
        match s.trim() {
            "Dry" => Ok(WeatherCondition::Dry),
            "Damp" => Ok(WeatherCondition::Damp),
            "Wet" => Ok(WeatherCondition::Wet),
            "HeavyRain" => Ok(WeatherCondition::HeavyRain),
            other => Err(format!("WeatherCondition invalido: '{other}'")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrackType {
    Road,
    Roval,
}

impl TrackType {
    pub fn as_str(&self) -> &str {
        match self {
            TrackType::Road => "Road",
            TrackType::Roval => "Roval",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "Roval" => TrackType::Roval,
            _ => TrackType::Road,
        }
    }
}

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

    pub fn from_str(s: &str) -> Self {
        match s {
            "Finalizada" => SeasonStatus::Finalizada,
            "Ativa" | "EmAndamento" => SeasonStatus::EmAndamento,
            _ => SeasonStatus::EmAndamento,
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

    pub fn from_str(s: &str) -> Self {
        match s {
            "Concluida" => RaceStatus::Concluida,
            _ => RaceStatus::Pendente,
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

// ── Tipo de incidente ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum IncidentType {
    Colisao,
    Toque,
    SpinOut,
    FalhaMecanica,
    ErroPit,
}

impl IncidentType {
    pub fn as_str(&self) -> &str {
        match self {
            IncidentType::Colisao => "Colisao",
            IncidentType::Toque => "Toque",
            IncidentType::SpinOut => "SpinOut",
            IncidentType::FalhaMecanica => "FalhaMecanica",
            IncidentType::ErroPit => "ErroPit",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "Colisao" => IncidentType::Colisao,
            "Toque" => IncidentType::Toque,
            "SpinOut" => IncidentType::SpinOut,
            "FalhaMecanica" => IncidentType::FalhaMecanica,
            _ => IncidentType::ErroPit,
        }
    }
}

// ── Severidade do incidente ───────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum IncidentSeverity {
    Leve,
    Moderado,
    Grave,
}

impl IncidentSeverity {
    pub fn as_str(&self) -> &str {
        match self {
            IncidentSeverity::Leve => "Leve",
            IncidentSeverity::Moderado => "Moderado",
            IncidentSeverity::Grave => "Grave",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "Moderado" => IncidentSeverity::Moderado,
            "Grave" => IncidentSeverity::Grave,
            _ => IncidentSeverity::Leve,
        }
    }
}

// ── Segmento de corrida ───────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RaceSegment {
    Largada,
    Abertura,
    Desenvolvimento,
    RetaFinal,
    Conclusao,
}

impl RaceSegment {
    pub fn as_str(&self) -> &str {
        match self {
            RaceSegment::Largada => "Largada",
            RaceSegment::Abertura => "Abertura",
            RaceSegment::Desenvolvimento => "Desenvolvimento",
            RaceSegment::RetaFinal => "RetaFinal",
            RaceSegment::Conclusao => "Conclusao",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "Largada" => RaceSegment::Largada,
            "Abertura" => RaceSegment::Abertura,
            "Desenvolvimento" => RaceSegment::Desenvolvimento,
            "RetaFinal" => RaceSegment::RetaFinal,
            _ => RaceSegment::Conclusao,
        }
    }
}

// ── Tipo de lesão ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum InjuryType {
    Leve,
    Moderada,
    Grave,
    Critica,
}

impl InjuryType {
    pub fn as_str(&self) -> &str {
        match self {
            InjuryType::Leve => "Leve",
            InjuryType::Moderada => "Moderada",
            InjuryType::Grave => "Grave",
            InjuryType::Critica => "Critica",
        }
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

// ── Fase da temporada ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SeasonPhase {
    BlocoRegular,
    JanelaConvocacao,
    BlocoEspecial,
    /// Fase de encerramento após o bloco especial: desmontagem administrativa
    /// (expiração de contratos especiais, limpeza de lineups) e repercussões.
    /// Segue BlocoEspecial e precede advance_season.
    PosEspecial,
}

impl SeasonPhase {
    pub fn as_str(&self) -> &str {
        match self {
            SeasonPhase::BlocoRegular => "BlocoRegular",
            SeasonPhase::JanelaConvocacao => "JanelaConvocacao",
            SeasonPhase::BlocoEspecial => "BlocoEspecial",
            SeasonPhase::PosEspecial => "PosEspecial",
        }
    }

    /// Parser estrito para leitura de banco de dados.
    pub fn from_str_strict(s: &str) -> Result<Self, String> {
        match s.trim() {
            "BlocoRegular" => Ok(SeasonPhase::BlocoRegular),
            "JanelaConvocacao" => Ok(SeasonPhase::JanelaConvocacao),
            "BlocoEspecial" => Ok(SeasonPhase::BlocoEspecial),
            "PosEspecial" => Ok(SeasonPhase::PosEspecial),
            other => Err(format!("SeasonPhase inválido: '{other}'")),
        }
    }
}

impl std::fmt::Display for SeasonPhase {
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

// ── Dificuldade ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Difficulty {
    Facil,
    Medio,
    Dificil,
    Lendario,
}

impl Difficulty {
    pub fn as_str(&self) -> &str {
        match self {
            Difficulty::Facil => "Facil",
            Difficulty::Medio => "Medio",
            Difficulty::Dificil => "Dificil",
            Difficulty::Lendario => "Lendario",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "Facil" => Difficulty::Facil,
            "Dificil" => Difficulty::Dificil,
            "Lendario" => Difficulty::Lendario,
            _ => Difficulty::Medio,
        }
    }
}
