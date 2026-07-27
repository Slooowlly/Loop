//! DTOs da equipe: resumo, classificação de construtores, finanças e dossiê histórico.

use serde::{Deserialize, Serialize};

use crate::commands::race_history::TrophyInfo;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamSummary {
    pub id: String,
    pub nome: String,
    pub nome_curto: String,
    pub cor_primaria: String,
    pub cor_secundaria: String,
    pub categoria: String,
    #[serde(default)]
    pub classe: Option<String>,
    pub car_performance: f64,
    /// Nível do Carro (1–10) do Sistema de Nível do Carro — a ÚNICA leitura de carro que
    /// o jogador vê. Derivado das 11 peças (média dos níveis).
    #[serde(default)]
    pub car_level: u8,
    pub confiabilidade: f64,
    #[serde(default)]
    pub pit_strategy_risk: f64,
    #[serde(default)]
    pub pit_crew_quality: f64,
    pub budget: f64,
    #[serde(default)]
    pub spending_power: f64,
    #[serde(default)]
    pub salary_ceiling: f64,
    #[serde(default)]
    pub budget_index: f64,
    #[serde(default)]
    pub cash_balance: f64,
    #[serde(default)]
    pub debt_balance: f64,
    #[serde(default)]
    pub financial_state: String,
    #[serde(default)]
    pub season_strategy: String,
    #[serde(default)]
    pub last_round_income: f64,
    #[serde(default)]
    pub last_round_expenses: f64,
    #[serde(default)]
    pub last_round_net: f64,
    #[serde(default)]
    pub parachute_payment_remaining: f64,
    pub piloto_1_id: Option<String>,
    pub piloto_1_nome: Option<String>,
    #[serde(default)]
    pub piloto_1_salario_anual: Option<f64>,
    pub piloto_2_id: Option<String>,
    pub piloto_2_nome: Option<String>,
    #[serde(default)]
    pub piloto_2_salario_anual: Option<f64>,
    /// Política interna da garagem (módulo `hierarchy`). Quem é N1 aqui é a hierarquia
    /// REAL — pode divergir de `piloto_1_id`/`piloto_2_id`, que é só a ordem dos slots,
    /// depois de uma inversão no meio da temporada.
    #[serde(default)]
    pub hierarquia_n1_id: Option<String>,
    #[serde(default)]
    pub hierarquia_n2_id: Option<String>,
    /// Clima: `estavel` | `competitivo` | `tensao` | `reavaliacao` | `inversao` | `crise`.
    #[serde(default)]
    pub hierarquia_status: String,
    /// Tensão acumulada (0–100). Acima de 50 já pesa na moral da equipe
    /// ([`crate::finance::morale::advance_team_morale`]).
    #[serde(default)]
    pub hierarquia_tensao: f64,
    #[serde(default)]
    pub hierarquia_inversoes_temporada: i32,
    /// PRESENÇA PÚBLICA da equipe (0–100): perfil de audiência derivado da mídia do
    /// lineup ativo (`public_presence::team`, 70% do mais midiático + 30% do segundo).
    /// Multiplica linearmente o patrocínio da rodada em `commands/race/financas.rs` —
    /// é o que torna a escolha de companheiro uma decisão financeira.
    ///
    /// NÃO confundir com `DriverCareerPresenceBlock.presenca` (piloto.rs), que é TEMPO
    /// DE CARREIRA, nem com `market::visibility::MarketVisibilityTier`, que classifica
    /// a `midia` de um piloto individual. Grandezas distintas e sem tier aqui.
    #[serde(default)]
    pub presenca_publica: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamStanding {
    pub posicao: i32,
    pub id: String,
    pub nome: String,
    pub nome_curto: String,
    pub cor_primaria: String,
    #[serde(default)]
    pub cash_balance: f64,
    #[serde(default)]
    pub car_performance: f64,
    /// Nível do Carro (1–10) — Sistema de Nível do Carro (a leitura de carro do jogador).
    #[serde(default)]
    pub car_level: u8,
    /// Confiabilidade (0–100) e qualidade do pit crew (0–100) da equipe — leituras reais do
    /// modelo Team, para o comparativo de gestão/performance no ranking da categoria.
    #[serde(default)]
    pub confiabilidade: f64,
    #[serde(default)]
    pub pit_crew_quality: f64,
    #[serde(default)]
    pub founded_year: i32,
    pub pontos: i32,
    pub vitorias: i32,
    pub piloto_1_nome: Option<String>,
    pub piloto_1_tenure_seasons: Option<i32>,
    pub piloto_2_nome: Option<String>,
    pub piloto_2_tenure_seasons: Option<i32>,
    pub trofeus: Vec<TrophyInfo>,
    pub classe: Option<String>,
    pub temp_posicao: i32,
    pub categoria_anterior: Option<String>,
    /// Históricos acumulados de carreira da equipe (reais, do modelo Team). Alimentam o
    /// ranking e o fallback do drawer sem estimativas fabricadas no front.
    #[serde(default)]
    pub historico_vitorias: i32,
    #[serde(default)]
    pub historico_podios: i32,
    #[serde(default)]
    pub historico_titulos_construtores: i32,
}

/// Resposta do comando `get_team_finance_report`: a divisão financeira REAL da equipe,
/// lida da tabela `team_finance_history`. Números crus (o front cuida de rótulos, cores
/// e formatação). Substitui os valores fabricados na aba My Team.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TeamFinanceReport {
    /// Total de rodadas com histórico registrado (0 = save antigo / sem corridas ainda).
    pub rounds_recorded: i32,
    /// Última rodada com histórico (para os ledgers de entradas/saídas).
    pub latest: Option<TeamFinanceRound>,
    /// Acumulado da temporada corrente (para a rosca de custos e a leitura de receita).
    /// Aqui `round` carrega a CONTAGEM de rodadas somadas na temporada.
    pub season: Option<TeamFinanceRound>,
    /// Caixa ao fim de cada rodada recente, em ordem cronológica (para o gráfico de caixa).
    pub cash_timeline: Vec<TeamFinanceCashPoint>,
    /// Prêmio de construtores ESTIMADO se a temporada terminasse agora (pela posição atual
    /// no campeonato). Só projeção de exibição — NÃO entra no caixa nem nas decisões da IA.
    pub expected_constructor_prize: f64,
    /// Posição atual da equipe no campeonato de construtores (0 = indisponível).
    pub current_position: i32,
    /// Nº de equipes no grupo do campeonato (para contextualizar a posição/projeção).
    pub grid_size: i32,
}

/// As 9 linhas reais de receita/despesa + totais. Serve tanto para a última rodada
/// quanto para o acumulado da temporada.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TeamFinanceRound {
    pub season_number: i32,
    pub round: i32,
    pub sponsorship_income: f64,
    /// Bilheteria/portão da rodada (Fase 3 do Estrelato): público que a fama do lineup
    /// atrai, escalado pelo prestígio do evento. 0 nas linhas legadas.
    pub gate_income: f64,
    pub result_bonus: f64,
    pub partial_prize_income: f64,
    pub aid_income: f64,
    pub salary_expense: f64,
    pub event_operations_cost: f64,
    pub structural_maintenance_cost: f64,
    pub technical_investment_cost: f64,
    pub debt_service_cost: f64,
    /// Prêmio de construtores creditado (só > 0 na linha de encerramento da temporada).
    pub constructor_prize_income: f64,
    pub income_total: f64,
    pub expenses_total: f64,
    pub net: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TeamFinanceCashPoint {
    pub season_number: i32,
    pub round: i32,
    pub cash_balance: f64,
    pub net: f64,
    /// `true` na linha de ENCERRAMENTO (prêmio de construtores) — o front rotula/colore
    /// esse ponto de forma distinta no gráfico de caixa.
    pub is_season_close: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamHistoryDossier {
    pub team_id: String,
    pub category: String,
    pub record_scope: String,
    pub has_history: bool,
    pub records: Vec<TeamHistoryRecord>,
    pub sport: TeamHistorySport,
    pub identity: TeamHistoryIdentity,
    pub management: TeamHistoryManagement,
    pub timeline: Vec<TeamHistoryTimelineItem>,
    pub title_categories: Vec<TeamHistoryTitleCategory>,
    pub category_path: Vec<TeamHistoryCategoryStep>,
    /// Eventos de propriedade/diretoria (ex.: venda por colapso). Consumido pelas
    /// abas Identidade (como "eras" do time) e Gestão (como evento financeiro).
    pub ownership_events: Vec<TeamHistoryOwnershipEvent>,
    /// Superlativos da equipe (melhor temporada, maior sequência de títulos...).
    /// Enriquecem a aba Record além dos rankings comparativos.
    pub highlights: Vec<TeamHistoryHighlight>,
    /// Marcos da história (primeira vitória, primeiro título...) com o ano.
    pub milestones: Vec<TeamHistoryMilestone>,
    /// Resultados temporada a temporada — alimenta a aba Esportivo (resultados
    /// ao longo do tempo), distinguindo-a de Categorias (movimento entre tiers).
    pub season_results: Vec<TeamHistorySeasonResult>,
    /// Resumo de movimento entre categorias (real) — alimenta a aba Categorias.
    pub movement: TeamHistoryMovement,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TeamHistoryMovement {
    pub promotions: i32,
    pub relegations: i32,
    pub time_by_category: String,
    pub best_category: String,
    pub hardest_category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamHistoryHighlight {
    pub label: String,
    pub value: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamHistoryMilestone {
    pub label: String,
    pub year: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamHistorySeasonResult {
    pub year: String,
    pub category: String,
    /// Posição final no campeonato naquela temporada ("P3", "—" se desconhecida).
    pub position: String,
    pub wins: i32,
    pub podiums: i32,
    pub points: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamHistoryOwnershipEvent {
    pub year: String,
    pub event_type: String,
    /// Título curto para a aba Identidade (ex.: "Nova diretoria").
    pub title: String,
    /// Descrição contextual.
    pub detail: String,
    /// Resumo financeiro para a aba Gestão (ex.: "Dívida de $38M zerada · aporte de $7M").
    pub financial_note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamHistoryRecord {
    pub label: String,
    pub rank: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamHistorySport {
    pub seasons: String,
    pub current_streak: String,
    pub best_streak: String,
    pub podium_rate: String,
    pub win_rate: String,
    pub races: i32,
    pub wins: i32,
    pub podiums: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamHistoryIdentity {
    pub origin: String,
    pub current: String,
    /// Herança por experiência real (temporadas): Novata → Tradicional.
    pub heritage: String,
    pub profile: String,
    pub summary: String,
    pub rival: TeamHistoryRival,
    pub symbol_driver: String,
    pub symbol_driver_detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamHistoryRival {
    pub name: String,
    pub current_category: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamHistoryManagement {
    pub operation_health: String,
    pub peak_cash: String,
    pub worst_crisis: String,
    pub healthy_years: String,
    pub efficiency: String,
    pub biggest_investment: String,
    pub summary: String,
    pub peak_cash_detail: String,
    pub worst_crisis_detail: String,
    pub healthy_years_detail: String,
    pub efficiency_detail: String,
    pub investment_detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamHistoryTimelineItem {
    pub year: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamHistoryTitleCategory {
    pub category: String,
    pub year: String,
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamHistoryCategoryStep {
    pub category: String,
    pub years: String,
    pub detail: String,
    pub color: String,
    /// "start" | "promotion" | "relegation" | "same" — marcador da escada.
    pub movement: String,
}
