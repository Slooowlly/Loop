//! Tipos serializáveis da pré-temporada: fase, estado, eventos de mercado,
//! resultado da semana e o plano com suas ações pendentes.

use super::*;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PreSeasonPhase {
    ContractExpiry,
    Transfers,
    PlayerProposals,
    RookiePlacement,
    Finalization,
    Complete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreSeasonState {
    pub season_number: i32,
    pub current_week: i32,
    pub total_weeks: i32,
    pub phase: PreSeasonPhase,
    pub is_complete: bool,
    pub player_has_pending_proposals: bool,
    /// Verdadeiro se o jogador já tem um contrato regular ativo para esta temporada.
    #[serde(default)]
    pub player_has_team: bool,
    #[serde(default)]
    pub current_display_date: Option<String>,
    /// Primeira semana que contrata alguém. Antes dela a janela é foto (1) e saídas (2),
    /// e o jogador não recebe proposta nenhuma — só a expectativa abaixo. Vem do estado
    /// (e não de uma constante repetida no front) pra a UI não duplicar o número.
    #[serde(default = "default_signings_start_week")]
    pub signings_start_week: i32,
    /// Quantas equipes devem cortejar o jogador quando o mercado abrir. Só existe
    /// enquanto ele é agente livre e o mercado ainda não contrata; some na semana em
    /// que as propostas de verdade começam. Ver [`PlayerInterestForecast`].
    #[serde(default)]
    pub player_interest_forecast: Option<PlayerInterestForecast>,
}

/// Expectativa de procura pelo jogador nas semanas de abertura: uma FAIXA, não um
/// número — na semana 1 ainda nem se sabe quais assentos vão abrir, então a estimativa
/// sai dos contratos que vencem e aperta na semana 2, quando as vagas já são reais.
/// O número da semana 3 é o que vira proposta, então a faixa tem que conter a verdade.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlayerInterestForecast {
    pub min: i32,
    pub max: i32,
}

pub(super) fn default_signings_start_week() -> i32 {
    i32::from(crate::constants::timeline::MARKET_SIGNINGS_START_WEEK)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MarketEventType {
    ContractExpired,
    ContractRenewed,
    TransferCompleted,
    TransferRejected,
    RookieSigned,
    PlayerProposalReceived,
    HierarchyUpdated,
    PreSeasonComplete,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketEvent {
    pub event_type: MarketEventType,
    pub headline: String,
    pub description: String,
    pub driver_id: Option<String>,
    pub driver_name: Option<String>,
    pub team_id: Option<String>,
    pub team_name: Option<String>,
    pub from_team: Option<String>,
    pub to_team: Option<String>,
    pub categoria: Option<String>,
    #[serde(default)]
    pub from_categoria: Option<String>,
    #[serde(default)]
    pub movement_kind: Option<String>,
    #[serde(default)]
    pub championship_position: Option<i32>,
    #[serde(default)]
    pub seasons_at_previous: Option<i32>,
    /// Vínculo do piloto deste evento com o JOGADOR, p/ o feed dar ênfase:
    /// `"rival"` (rivalidade ativa) | `"raced"` (já correu contra você E entrou na
    /// sua categoria atual) | `"favorite"` (favoritado — reservado, sem sistema ainda).
    /// `None` = sem vínculo (maioria do feed). Prioridade: favorite > rival > raced.
    #[serde(default)]
    pub relation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeekResult {
    pub week_number: i32,
    pub phase: PreSeasonPhase,
    pub events: Vec<MarketEvent>,
    pub is_last_week: bool,
    pub player_proposals: Vec<MarketProposal>,
    pub remaining_vacancies: i32,
    pub next_phase: PreSeasonPhase,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreSeasonPlan {
    pub state: PreSeasonState,
    pub planned_events: Vec<PlannedEvent>,
    pub executed_weeks: Vec<WeekResult>,
    /// Dispensas capturadas no início da janela. Hoje as pré-passes rodam ao SAIR da
    /// semana 1 e as dispensas saem no feed na mesma hora, então este campo nasce vazio;
    /// ele fica porque saves de janelas antigas (que capturavam na abertura) ainda
    /// carregam eventos aqui, e perdê-los apagaria o feed da semana deles.
    #[serde(default)]
    pub pending_departures: Vec<MarketEvent>,
    /// Renovações confirmadas pelas pré-passes, emitidas ao sair da semana 2. Ficam
    /// separadas das dispensas de propósito: dispensa MOVE o piloto (o grid muda entre a
    /// semana 1 e a 2, e o feed tem que explicar a mudança), renovação não move ninguém
    /// — é a confirmação de quem ficou, e cabe na semana em que nada se mexe.
    #[serde(default)]
    pub pending_renewals: Vec<MarketEvent>,
    /// Se as pré-passes (expiração, renovação, mérito, assédio) já caíram nesta janela.
    /// Default `true` de propósito: save de uma janela anterior já as teve aplicadas na
    /// abertura, e um default `false` as rodaria de novo — dispensando meio grid duas vezes.
    #[serde(default = "verdadeiro")]
    pub prepasses_applied: bool,
    /// Se a passada de MOVIMENTOS (rebaixamento por mérito, assédio, campeão do rookie)
    /// já caiu. Ela é adiada até a semana em que o mercado abre, porque tira o piloto de
    /// uma equipe e o põe em outra — coisa que as semanas de abertura não podem mostrar.
    /// Mesmo default `true` das pré-passes, e pelo mesmo motivo: save de janela antiga já
    /// as teve aplicadas, e rodá-las de novo transferiria gente duas vezes.
    #[serde(default = "verdadeiro")]
    pub movements_applied: bool,
    /// Categoria de cada piloto no INÍCIO da pré-temporada (antes das pré-passes, que
    /// limpam o `categoria_atual` dos dispensados) — origem p/ promovido/rebaixado.
    #[serde(default)]
    pub category_snapshot: std::collections::HashMap<String, String>,
    /// Equipe anterior de cada piloto no INÍCIO da pré-temporada (antes das pré-passes):
    /// driver_id → (nome da equipe, temporadas consecutivas nela). Origem p/ o popup de
    /// detalhe da transferência (de qual equipe veio + quanto tempo ficou lá).
    #[serde(default)]
    pub previous_team: std::collections::HashMap<String, (String, i32)>,
    /// Proposta de QUEBRA DE CONTRATO ao jogador (Fase 2b.3): computada uma vez no
    /// setup (rara), mostrada como o leilão ao vivo e limpa quando ele decide. None
    /// na esmagadora maioria das janelas.
    #[serde(default)]
    pub player_poach_offer: Option<crate::market::pipeline::PlayerPoachOffer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlannedEvent {
    pub week: i32,
    pub event: PendingAction,
    pub executed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
// Enum serializado para o plano de pré-temporada; as variantes carregam contexto
// editorial completo e boxear só para uniformizar o tamanho não compensa.
#[allow(clippy::large_enum_variant)]
pub enum PendingAction {
    PhaseMarker {
        phase: PreSeasonPhase,
    },
    ExpireContract {
        contract_id: String,
        driver_id: String,
        driver_name: String,
        team_id: String,
        team_name: String,
    },
    RenewContract {
        driver_id: String,
        driver_name: String,
        team_id: String,
        team_name: String,
        new_salary: f64,
        new_duration: i32,
        new_role: String,
    },
    Transfer {
        driver_id: String,
        driver_name: String,
        from_team_id: Option<String>,
        from_team_name: Option<String>,
        #[serde(default)]
        from_categoria: Option<String>,
        to_team_id: String,
        to_team_name: String,
        salary: f64,
        duration: i32,
        role: String,
    },
    PlayerProposal {
        proposal: MarketProposal,
    },
    PlaceRookie {
        driver: Driver,
        team_id: String,
        team_name: String,
        salary: f64,
        duration: i32,
        role: String,
    },
    UpdateHierarchy {
        team_id: String,
        team_name: String,
        n1_id: Option<String>,
        n1_name: String,
        n2_id: Option<String>,
        n2_name: String,
        // Estado hierárquico anterior (capturado no início da preseason).
        // #[serde(default)] para compatibilidade com saves anteriores que não têm esses campos.
        #[serde(default)]
        prev_n1_id: Option<String>,
        #[serde(default)]
        prev_n2_id: Option<String>,
        #[serde(default)]
        prev_tensao: f64,
        #[serde(default = "default_estavel")]
        prev_status: String,
        #[serde(default)]
        prev_categoria: String,
    },
}

pub(super) fn default_estavel() -> String {
    "estavel".to_string()
}

pub(super) fn verdadeiro() -> bool {
    true
}
