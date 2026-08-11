//! DTOs do mercado de pilotos expostos à UI (prévia de agentes livres).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FreeAgentPreview {
    pub driver_id: String,
    pub driver_name: String,
    pub categoria: String,
    pub is_rookie: bool,
    pub previous_team_name: Option<String>,
    pub previous_team_color: Option<String>,
    pub previous_team_abbr: Option<String>,
    pub seasons_at_last_team: i32,
    pub total_career_seasons: i32,
    pub license_nivel: String,
    pub license_sigla: String,
    pub last_championship_position: Option<i32>,
    pub last_championship_total_drivers: Option<i32>,
    /// Tier de prestígio (0=Rookie … 6=Endurance) da categoria onde ele corre hoje.
    /// É a chave de agrupamento da coluna (faixa de nível). `None` = rookie/sem categoria.
    pub market_tier: Option<u8>,
    /// Temporadas parado (ver `FreeAgentRaw::seasons_idle`). Usado pelo marcador "parado".
    pub seasons_idle: Option<i32>,
    /// IDs das categorias onde ele pode realmente pegar vaga (mesma regra do leilão:
    /// tier ±1 + licença exigida, com +1 de promoção liberado). Usado pelo filtro do topo.
    pub eligible_categories: Vec<String>,
}

/// O que o JOGADOR já viveu com um piloto que ficou sem vaga.
///
/// A lista de deslocados sozinha é uma lista de estranhos: seis nomes que o
/// jogador nunca ouviu falar, e no meio deles um que ele bateu na última volta
/// de Interlagos. Este DTO é o que separa os dois casos.
///
/// Só vem preenchido para quem realmente dividiu grid com o jogador — quem nunca
/// cruzou com ele fica com tudo em zero, e a UI não desenha nada.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DisplacedDriverContext {
    pub driver_id: String,
    /// Corridas em que os dois têm resultado: dividiram o grid.
    pub shared_races: i32,
    /// Dessas, quantas o jogador terminou à frente. Abandono dos dois lados fica
    /// de fora do placar — quebrar o motor não é perder um duelo.
    pub player_ahead: i32,
    pub driver_ahead: i32,
    /// Número da última temporada em que se encontraram.
    pub last_shared_season: Option<i32>,
    /// `"nemesis"` | `"rival"` | `None`, com o MESMO critério das outras telas —
    /// sai de `select_player_interests`, não de um limiar próprio. Duas definições
    /// de "quem é rival" no mesmo jogo divergem na primeira vez que uma muda.
    pub rival_role: Option<String>,
}

/// Um assento VAZIO no mundo, como o painel de mercado em temporada o lê.
///
/// A pergunta que ele responde não é "quem está sem equipe" (isso é agente livre,
/// e é `FreeAgentPreview`): é "onde existe cadeira aberta hoje, e alguma delas é
/// para mim". Por isso cada vaga carrega o veredito de elegibilidade em vez de a
/// tela recalcular a regra em JS — a regra de licença e de faixa de tier é a
/// mesma da proposta emergencial, e duas cópias divergem na primeira mudança.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenSeat {
    pub team_id: String,
    pub team_name: String,
    pub team_color: String,
    /// Chave crua da categoria (`gt3`, `endurance`) — a tela nomeia.
    pub categoria: String,
    /// Classe dentro da categoria compartilhada (`gt3`, `lmp2`), quando há.
    pub classe: Option<String>,
    /// Tier de prestígio da categoria da vaga. `None` quando a categoria não tem
    /// config — a mesma ausência que o resto do mercado trata como tier 0.
    pub categoria_tier: Option<u8>,
    /// `"numero_1"` | `"numero_2"`, na convenção do `TeamRole`.
    pub papel: String,
    /// Carro da equipe normalizado em 0–100, a MESMA escala do card de oferta.
    pub car_performance_rating: u8,
    /// A licença do jogador cobre a divisão desta vaga.
    pub licenca_ok: bool,
    /// A vaga está na faixa de tier em que o mercado ofertaria ao jogador (o tier
    /// dele ou um degrau acima), pelo mesmo critério de
    /// `generate_emergency_player_proposals`.
    pub tier_ok: bool,
    /// O que esta equipe pagaria ao jogador neste assento. Só vem quando a vaga é
    /// elegível — estimar salário de um assento que ele não pode ocupar seria
    /// inventar uma oferta que o mercado nunca faria.
    pub salario_estimado: Option<f64>,
}

/// O painel de mercado do jogador FORA da janela de pré-temporada.
///
/// Responde a pergunta que o meio da temporada não tinha onde responder: que
/// cadeira está aberta no mundo, e qual delas é para mim.
///
/// O estado do CONTRATO não vem aqui de propósito: ele já viaja completo em
/// `get_driver_detail(jogador).contrato_mercado`, e a tela que abre este painel
/// carrega aquele payload de qualquer forma. Um segundo caminho para o mesmo
/// contrato é a receita para os dois discordarem.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeasonMarketBoard {
    /// Categoria efetiva do jogador hoje. `None` para agente livre.
    pub player_categoria: Option<String>,
    /// Tier da categoria acima. `None` quando não há categoria resolvível — e a
    /// tela não desenha a faixa em vez de assumir tier 0.
    pub player_tier: Option<u8>,
    /// Assentos vazios do mundo, os elegíveis primeiro e, dentro de cada grupo, do
    /// melhor carro para o pior.
    pub vagas: Vec<OpenSeat>,
    /// Quantas das vagas acima são elegíveis para o jogador (licença E faixa).
    pub vagas_elegiveis: i32,
}
