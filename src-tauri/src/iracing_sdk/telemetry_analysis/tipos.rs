//! DTOs da análise de telemetria: os cards (ritmo, rival, erro, melhor momento), as
//! séries dos gráficos e o agregado `TelemetryAnalysis` que cruza para o front.

use serde::Serialize;

use super::combustivel::FuelSummary;
use super::setores::SectorAnalysis;

/// Análise de RITMO + consistência do jogador (tempos em ms).
#[derive(Debug, Clone, Default, Serialize)]
pub struct PaceAnalysis {
    pub best_lap_ms: f64,
    /// Ritmo médio REAL (todas as voltas).
    pub real_avg_ms: f64,
    /// Ritmo LIMPO (voltas dentro de 4% da melhor — sem erros grosseiros).
    pub clean_avg_ms: f64,
    /// Tempo perdido por erros/tráfego por volta (real − limpo).
    pub lost_per_lap_ms: f64,
    /// Ritmo médio do CAMPO (todos os carros) — para comparar.
    pub grid_avg_ms: f64,
    /// Você vs campo (limpo − campo); negativo = mais rápido que a média.
    pub vs_grid_ms: f64,
    /// Voltas "boas" (dentro de 4% da melhor) / total.
    pub good_laps: i32,
    pub total_laps: i32,
    /// A consistência só é confiável com voltas suficientes (>= 3 válidas).
    /// Abaixo disso a tela esconde o card de consistência.
    pub consistency_reliable: bool,
    /// Quantas voltas do CAMPO entraram na média do grid (confiabilidade do vs_grid).
    pub grid_sample: i32,
    /// O "vs grid" só é confiável com amostra suficiente do campo.
    pub vs_grid_reliable: bool,
}

/// Movimentação BRUTA de posição do jogador na pista (Nível 2 do breakdown).
/// É só a trajetória observada — o SALDO oficial (grid → chegada) e as "herdadas
/// por DNF" continuam vindo da tabela oficial. Tudo aqui é ESTIMADO (amostragem).
#[derive(Debug, Clone, Serialize)]
pub struct PositionFlow {
    /// Soma das SUBIDAS de posição observadas na pista (ganhos brutos).
    pub gained_on_track: i32,
    /// Soma das QUEDAS de posição observadas na pista (perdas brutas).
    pub lost_on_track: i32,
    /// Amostras de posição usadas — base da confiança.
    pub samples: i32,
}

/// Sinais de incidente/abandono do jogador que NÃO estão no `RaceHistory` —
/// vêm do monitor ao vivo (`Attempt.crashes`, DNF). O chamador preenche.
#[derive(Debug, Clone, Default)]
pub struct PlayerIncidents {
    /// Voltas em que o monitor flagrou batida/contato do jogador.
    pub crash_laps: Vec<i32>,
    /// O jogador abandonou a prova.
    pub is_dnf: bool,
    /// Volta em que a corrida do jogador encerrou (última volta / batida).
    pub dnf_lap: Option<i32>,
}

/// O "erro mais caro" da corrida (2b-2): a volta de maior custo estimado. Sempre
/// ESTIMADO — combina volta lenta vs ritmo limpo, posições perdidas e incidente.
/// `kind`: "incident" | "pace_drop" | "position_loss" | "dnf". A tela formata a
/// frase a partir destes números. Só existe com confiança >= média (baixa some).
#[derive(Debug, Clone, Serialize)]
pub struct CostlyMistake {
    pub lap: i32,
    pub kind: String,
    /// Tempo perdido estimado vs ritmo limpo (ms). 0 quando n/a.
    pub time_lost_ms: f64,
    /// Posições perdidas nessa volta. 0 quando n/a.
    pub positions_lost: i32,
    /// "alta" | "media" (baixa nunca chega aqui — escondemos).
    pub confidence: String,
}

/// O piloto com quem você mais brigou.
#[derive(Debug, Clone, Serialize)]
pub struct RivalCard {
    pub pilot_name: String,
    /// Voltas distintas em que ele esteve ao seu lado (à frente ou atrás).
    pub laps_battled: i32,
    /// Gap médio para ele nesses momentos (segundos).
    pub avg_gap_s: f64,
}

/// O melhor momento da corrida (2b-3): o espelho positivo do erro mais caro.
/// `kind`: "position_gain" | "rival_beaten" | "recovery" | "clean_streak" |
/// "best_lap". Escolhido por PRIORIDADE (ganho de pos > rival > recuperação >
/// sequência > melhor volta como fallback), não só por score. Só com confiança
/// >= média; corrida sem destaque real → None (não força narrativa bonita).
#[derive(Debug, Clone, Serialize)]
pub struct BestMoment {
    /// Volta do momento (0 quando é multi-volta: sequência/rival).
    pub lap: i32,
    pub kind: String,
    pub positions_gained: i32,
    /// Ganho de tempo vs ritmo limpo (ms) — p/ volta forte / melhor volta.
    pub time_gain_ms: f64,
    /// Tamanho da sequência limpa / voltas de batalha com o rival.
    pub streak: i32,
    /// Nome do rival superado (kind "rival_beaten").
    pub rival_name: String,
    /// "alta" | "media".
    pub confidence: String,
}

/// Um ponto do race trace de um carro: gap ao líder + posição naquele instante.
/// `lap` é FRACIONÁRIO (volta do líder + progresso dele na volta), pra ultrapassagem
/// aparecer no ponto exato da volta em que aconteceu — não só na virada.
#[derive(Debug, Clone, Serialize)]
pub struct ChartTracePoint {
    pub lap: f64,
    pub gap: f64,
    pub position: i32,
}

/// A linha de um carro no race trace (legenda + destaque do jogador).
#[derive(Debug, Clone, Serialize)]
pub struct ChartCar {
    pub idx: i32,
    pub name: String,
    pub is_player: bool,
    pub points: Vec<ChartTracePoint>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChartLapTime {
    pub lap: i32,
    pub time_s: f64,
}

/// Tempo de volta de QUALQUER carro (para o seletor de ritmo por piloto).
#[derive(Debug, Clone, Serialize)]
pub struct ChartCarLapTime {
    pub idx: i32,
    pub lap: i32,
    pub time_s: f64,
}

/// Gap ao rival por volta, COM SINAL: + rival à frente (você caçando), − rival
/// atrás (você liderando a disputa).
#[derive(Debug, Clone, Serialize)]
pub struct ChartGap {
    pub lap: i32,
    pub gap_s: f64,
}

/// Séries para os gráficos da tela (2b — gráficos). Capturadas no import, então
/// não dependem do monitor ainda estar vivo. Vazio → a seção de gráficos some.
#[derive(Debug, Clone, Serialize)]
pub struct RaceCharts {
    /// Race trace: uma linha por carro (gap ao líder + posição por volta).
    pub cars: Vec<ChartCar>,
    /// Voltas sob bandeira amarela (faixas no gráfico).
    pub yellow_laps: Vec<i32>,
    /// Tempos de volta do jogador.
    pub lap_times: Vec<ChartLapTime>,
    /// Tempos de volta de TODOS os carros — para o seletor de ritmo por piloto.
    pub car_lap_times: Vec<ChartCarLapTime>,
    /// Gap ao rival por volta (vazio se não houve rival claro).
    pub rival_gap: Vec<ChartGap>,
    pub rival_name: String,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct TelemetryAnalysis {
    /// Houve telemetria utilizável (o jogador correu e foi monitorado).
    pub has_telemetry: bool,
    /// Quantas voltas do jogador o monitor capturou.
    pub laps_seen: i32,
    /// Voltas TOTAIS da corrida (do líder) — base para a confiança/cobertura.
    pub race_laps: i32,
    /// Última volta do jogador efetivamente capturada (p/ "telemetria até a volta X").
    pub last_lap_seen: i32,
    /// Confiança da análise: "alta" | "media" | "baixa".
    pub confidence: String,
    /// Cobertura incompleta — o jogador saiu bem antes do fim.
    pub is_partial: bool,
    pub pace: Option<PaceAnalysis>,
    pub rival: Option<RivalCard>,
    /// Fluxo de posições na pista (Nível 2 do breakdown) — None se faltam amostras.
    pub position_flow: Option<PositionFlow>,
    /// Erro mais caro (2b-2) — None numa corrida limpa (nada de inventar drama).
    pub mistake: Option<CostlyMistake>,
    /// Melhor momento (2b-3) — None se não houve destaque real.
    pub best_moment: Option<BestMoment>,
    /// Séries para os gráficos (race trace, tempos, gap ao rival). None se vazio.
    pub charts: Option<RaceCharts>,
    /// Estratégia de pneu inferida de TODOS os carros (paradas + clima). Vale mesmo
    /// se o jogador saiu cedo (a IA corre até o fim). Vazio se não houve paradas/clima.
    #[serde(default)]
    pub tire_strategies: Vec<crate::iracing_sdk::tire_strategy::CarTireStrategy>,
    /// Atalho: a estratégia de pneu do PRÓPRIO jogador (None se não identificada).
    #[serde(default)]
    pub player_tire: Option<crate::iracing_sdk::tire_strategy::CarTireStrategy>,
    /// Consumo de combustível do jogador (None se o SDK não deu o dado).
    #[serde(default)]
    pub fuel: Option<FuelSummary>,
    /// Parciais por setor do jogador (melhor por setor + setor fraco). None se
    /// faltam voltas completas.
    #[serde(default)]
    pub sectors: Option<SectorAnalysis>,
}
