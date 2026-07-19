//! Análise da TELEMETRIA da corrida (Fase 2 do pós-corrida): ritmo, consistência
//! e rival, a partir do histórico ao vivo do monitor (`RaceHistory`).
//!
//! Esses dados só existem ENQUANTO o jogador esteve na pista — se ele saiu cedo,
//! a análise vem parcial ou vazia. Tudo aqui é tolerante: campos `Option`/flags
//! para a tela "mostrar quando tem, esconder quando não tem". Nunca quebra.
//!
//! Lógica pura e testável; o chamador resolve `car_idx → nome` (via cars_meta +
//! roster) e passa o mapa.

use std::collections::HashMap;

use serde::Serialize;

use crate::iracing_sdk::race_monitor::{PlayerLap, RaceHistory};

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
    pub tire_strategies: Vec<super::tire_strategy::CarTireStrategy>,
    /// Atalho: a estratégia de pneu do PRÓPRIO jogador (None se não identificada).
    #[serde(default)]
    pub player_tire: Option<super::tire_strategy::CarTireStrategy>,
    /// Consumo de combustível do jogador (None se o SDK não deu o dado).
    #[serde(default)]
    pub fuel: Option<FuelSummary>,
    /// Parciais por setor do jogador (melhor por setor + setor fraco). None se
    /// faltam voltas completas.
    #[serde(default)]
    pub sectors: Option<SectorAnalysis>,
}

/// Análise dos 3 setores do jogador: melhor por setor + onde você mais perde.
#[derive(Debug, Clone, Serialize)]
pub struct SectorAnalysis {
    /// Melhor tempo por setor (ms): [S1, S2, S3].
    pub best_ms: [f64; 3],
    /// Volta teórica = soma dos melhores setores (ms).
    pub theoretical_best_ms: f64,
    /// Setor onde você mais perde vs seu próprio melhor (1..3). 0 = sem dado.
    pub weakest_sector: i32,
    /// Perda média nesse setor vs seu melhor dele (ms).
    pub weakest_loss_ms: f64,
}

/// Melhor por setor + setor fraco a partir dos parciais do jogador. Precisa de ≥2
/// parciais em CADA setor (senão o "melhor" é ruído de amostra única).
fn analyze_sectors(history: &RaceHistory) -> Option<SectorAnalysis> {
    let mut by_sector: [Vec<f64>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    for s in &history.player_sectors {
        if (1..=3).contains(&s.sector) && s.time > 0.0 {
            by_sector[(s.sector - 1) as usize].push(s.time * 1000.0);
        }
    }
    if by_sector.iter().any(|v| v.len() < 2) {
        return None;
    }
    let mut best_ms = [0.0f64; 3];
    let mut weakest_sector = 0;
    let mut weakest_loss_ms = 0.0;
    for i in 0..3 {
        let best = by_sector[i].iter().copied().fold(f64::INFINITY, f64::min);
        let avg = by_sector[i].iter().sum::<f64>() / by_sector[i].len() as f64;
        best_ms[i] = best;
        let loss = avg - best;
        if loss > weakest_loss_ms {
            weakest_loss_ms = loss;
            weakest_sector = (i + 1) as i32;
        }
    }
    Some(SectorAnalysis {
        best_ms,
        theoretical_best_ms: best_ms.iter().sum(),
        weakest_sector,
        weakest_loss_ms,
    })
}

/// Consumo de combustível do jogador na corrida (litros).
#[derive(Debug, Clone, Serialize)]
pub struct FuelSummary {
    /// Consumo médio por volta (L).
    pub used_per_lap_l: f64,
    /// Combustível restante na última volta captada (L).
    pub remaining_l: f64,
    /// Voltas de autonomia restantes no ritmo atual (remaining / used_per_lap).
    pub laps_left: f64,
}

/// Consumo do jogador a partir das voltas com combustível captado. Precisa de ≥2
/// voltas com `fuel_remaining >= 0` e consumo positivo (ruído/reabastecimento fora).
fn analyze_fuel(history: &RaceHistory) -> Option<FuelSummary> {
    let mut pts: Vec<(f64, f64)> = history
        .player_laps
        .iter()
        .filter(|l| l.fuel_remaining >= 0.0)
        .map(|l| (l.lap as f64, l.fuel_remaining))
        .collect();
    if pts.len() < 2 {
        return None;
    }
    pts.sort_by(|a, b| a.0.total_cmp(&b.0));
    let (lap0, fuel0) = pts[0];
    let (lap1, fuel1) = *pts.last().unwrap();
    let lap_span = lap1 - lap0;
    let burned = fuel0 - fuel1;
    if lap_span < 1.0 || burned <= 0.0 {
        return None;
    }
    let used_per_lap_l = burned / lap_span;
    let laps_left = if used_per_lap_l > 0.0 {
        fuel1 / used_per_lap_l
    } else {
        0.0
    };
    Some(FuelSummary {
        used_per_lap_l,
        remaining_l: fuel1,
        laps_left,
    })
}

fn mean(v: &[f64]) -> f64 {
    if v.is_empty() {
        0.0
    } else {
        v.iter().sum::<f64>() / v.len() as f64
    }
}

/// Limiar de volta "limpa": dentro de 4% da melhor volta.
const CLEAN_LAP_FACTOR: f64 = 1.04;

/// Voltas válidas mínimas para o card de RITMO aparecer.
const MIN_PACE_LAPS: i32 = 2;
/// Voltas válidas mínimas para a CONSISTÊNCIA ser confiável.
const MIN_CONSISTENCY_LAPS: i32 = 3;
/// Voltas mínimas do campo para o "vs grid" valer.
const MIN_GRID_SAMPLE: i32 = 3;
/// Voltas mínimas ao lado de um piloto para chamá-lo de RIVAL.
const MIN_RIVAL_LAPS: i32 = 3;
/// Gap médio máximo (s) para considerar que houve disputa real.
const MAX_RIVAL_GAP_S: f64 = 3.0;

// ── Sinais compactos de telemetria para o DOSSIÊ DE HABILIDADE (Fase 2) ───────
//
// O dossiê agrega POR CORRIDA, então guardamos escalares (não os pontos brutos):
// consistência de ritmo, fração de tempo em briga + saldo de posições na pista
// (racecraft avançado) e o ganho de posições na largada. Persistidos em
// `player_race_telemetry` no import e lidos depois pelo estimador (`player_skill`).

/// Gap (s) abaixo do qual consideramos que havia um carro "em cima" — briga.
const BATTLE_GAP_S: f64 = 1.0;
/// Amostras mínimas da batalha para a fração fazer sentido.
const MIN_BATTLE_POINTS: i32 = 10;
/// Janela da largada: comparamos a posição inicial com a de +20s.
const START_WINDOW_S: f64 = 20.0;
/// Span mínimo de amostras para o `start_delta` valer numa corrida curta.
const START_MIN_SPAN_S: f64 = 15.0;
/// Escala do coeficiente de variação → nota de consistência (cv 0 → 100).
const CONSISTENCY_CV_SCALE: f64 = 2000.0;

/// Sinais COMPACTOS de telemetria de uma corrida do jogador, para o dossiê de
/// habilidade (Fase 2). Um valor por corrida; o estimador agrega. Sentinela
/// `-1.0` (ou `start_valid=false`) = não computável nesta corrida.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerRaceTelemetry {
    pub laps_seen: i32,
    pub race_laps: i32,
    /// 0–100: quão parelhos foram os tempos de volta. -1 = poucas voltas.
    pub consistency: f64,
    /// 0–1: fração das amostras com um carro a < 1s. -1 = poucas amostras.
    pub battle_fraction: f64,
    /// Posições SUBIDAS na pista (bruto, do fluxo de posição).
    pub on_track_gained: i32,
    /// Posições PERDIDAS na pista (bruto).
    pub on_track_lost: i32,
    /// Posições ganhas nos ~20s iniciais (posição no start − posição em +20s).
    pub start_delta: i32,
    /// `start_delta` é confiável (houve amostra cobrindo a janela de largada).
    pub start_valid: bool,
}

/// Extrai os sinais do dossiê do histórico ao vivo + a análise já feita. `None`
/// quando não há nada utilizável (jogador não correu / não foi monitorado).
pub fn extract_player_race_telemetry(
    history: &RaceHistory,
    telemetry: &TelemetryAnalysis,
) -> Option<PlayerRaceTelemetry> {
    let times: Vec<f64> = history
        .player_laps
        .iter()
        .map(|l| l.time)
        .filter(|t| *t > 0.0)
        .collect();
    let laps_seen = times.len() as i32;
    let has_track = !history.player_track.is_empty();
    if laps_seen == 0 && !has_track {
        return None;
    }

    // Consistência: coeficiente de variação dos tempos de volta (menor = melhor).
    let consistency = if laps_seen >= MIN_CONSISTENCY_LAPS {
        let m = mean(&times);
        if m > 0.0 {
            let var = times.iter().map(|t| (t - m).powi(2)).sum::<f64>() / times.len() as f64;
            let cv = var.sqrt() / m;
            (100.0 - cv * CONSISTENCY_CV_SCALE).clamp(1.0, 100.0)
        } else {
            -1.0
        }
    } else {
        -1.0
    };

    // Fração de tempo em briga: amostras com um vizinho a < BATTLE_GAP_S.
    let mut battle = 0;
    let mut pts = 0;
    for p in &history.player_track {
        let mut nearest = f64::INFINITY;
        if p.ahead_idx >= 0 && p.gap_ahead.is_finite() && p.gap_ahead >= 0.0 {
            nearest = nearest.min(p.gap_ahead);
        }
        if p.behind_idx >= 0 && p.gap_behind.is_finite() && p.gap_behind >= 0.0 {
            nearest = nearest.min(p.gap_behind);
        }
        if nearest.is_finite() {
            pts += 1;
            if nearest < BATTLE_GAP_S {
                battle += 1;
            }
        }
    }
    let battle_fraction = if pts >= MIN_BATTLE_POINTS {
        battle as f64 / pts as f64
    } else {
        -1.0
    };

    // Saldo bruto de posições na pista (do fluxo de posição já calculado).
    let (on_track_gained, on_track_lost) = telemetry
        .position_flow
        .as_ref()
        .map(|f| (f.gained_on_track, f.lost_on_track))
        .unwrap_or((0, 0));

    let (start_delta, start_valid) = compute_start_delta(history);

    Some(PlayerRaceTelemetry {
        laps_seen,
        race_laps: telemetry.race_laps,
        consistency,
        battle_fraction,
        on_track_gained,
        on_track_lost,
        start_delta,
        start_valid,
    })
}

/// Posições ganhas na largada: posição na 1ª amostra vs. a de ~20s depois
/// (positivo = subiu). Fallback: numa corrida curta, usa a última amostra se o
/// span cobrir ao menos [`START_MIN_SPAN_S`].
fn compute_start_delta(history: &RaceHistory) -> (i32, bool) {
    let mut pts: Vec<(f64, i32)> = history
        .player_track
        .iter()
        .filter(|p| p.position > 0)
        .map(|p| (p.session_time, p.position))
        .collect();
    if pts.len() < 2 {
        return (0, false);
    }
    pts.sort_by(|a, b| a.0.total_cmp(&b.0));
    let (t0, pos0) = pts[0];
    if let Some(&(_, pos20)) = pts.iter().find(|(t, _)| *t >= t0 + START_WINDOW_S) {
        return (pos0 - pos20, true);
    }
    let (tl, posl) = *pts.last().unwrap();
    if tl - t0 >= START_MIN_SPAN_S {
        (pos0 - posl, true)
    } else {
        (0, false)
    }
}

/// Analisa o histórico. `name_by_idx`: car_idx → nome do piloto (resolvido fora).
/// `incidents`: sinais de batida/DNF do monitor (fora do `RaceHistory`).
pub fn analyze(
    history: &RaceHistory,
    name_by_idx: &HashMap<i32, String>,
    team_by_idx: &HashMap<i32, String>,
    incidents: &PlayerIncidents,
) -> TelemetryAnalysis {
    let player_idx = history.player_car_idx;
    let pace = analyze_pace(history, player_idx);
    let rival = analyze_rival(history, name_by_idx);
    let position_flow = analyze_position_flow(history);
    let mistake = analyze_mistake(history, incidents, pace.as_ref());
    let best_moment =
        analyze_best_moment(history, name_by_idx, pace.as_ref(), incidents, mistake.as_ref());
    let charts = build_charts(history, name_by_idx);
    let laps_seen = history.player_laps.len() as i32;
    let last_lap_seen = history
        .player_laps
        .iter()
        .map(|l| l.lap)
        .max()
        .unwrap_or(0);
    // Voltas totais da corrida = última volta do líder no race trace.
    let race_laps = history.laps.iter().map(|s| s.lap).max().unwrap_or(0);

    let (confidence, is_partial) = confidence_label(laps_seen, race_laps);

    // Estratégia de pneu (todos os carros) a partir das paradas + clima da corrida.
    // Resolve os nomes dos pilotos aqui (o módulo puro só conhece o car_idx).
    let mut tire_strategies = super::tire_strategy::infer_all(&history.pit_stops, history.weather);
    for s in &mut tire_strategies {
        s.pilot_name = name_by_idx
            .get(&s.car_idx)
            .cloned()
            .unwrap_or_else(|| format!("Carro {}", s.car_idx));
        s.team_name = team_by_idx.get(&s.car_idx).cloned().unwrap_or_default();
    }
    let player_tire = tire_strategies
        .iter()
        .find(|s| s.car_idx == player_idx)
        .cloned();

    TelemetryAnalysis {
        has_telemetry: pace.is_some()
            || rival.is_some()
            || position_flow.is_some()
            || mistake.is_some()
            || best_moment.is_some(),
        laps_seen,
        race_laps,
        last_lap_seen,
        confidence,
        is_partial,
        pace,
        rival,
        position_flow,
        mistake,
        best_moment,
        charts,
        tire_strategies,
        player_tire,
        fuel: analyze_fuel(history),
        sectors: analyze_sectors(history),
    }
}

/// Monta as séries dos gráficos a partir do histórico ao vivo. Resolve nomes via
/// `name_by_idx` (fallback "Carro N"). None se não há trace nem voltas.
fn build_charts(history: &RaceHistory, name_by_idx: &HashMap<i32, String>) -> Option<RaceCharts> {
    // Race trace: um conjunto de pontos por carro presente nos snapshots.
    let mut idx_set: std::collections::BTreeSet<i32> = std::collections::BTreeSet::new();
    for snap in &history.laps {
        for c in &snap.cars {
            idx_set.insert(c.idx);
        }
    }
    let cars: Vec<ChartCar> = idx_set
        .iter()
        .map(|&idx| {
            let points: Vec<ChartTracePoint> = history
                .laps
                .iter()
                .filter_map(|snap| {
                    snap.cars.iter().find(|c| c.idx == idx).map(|c| ChartTracePoint {
                        lap: snap.lap as f64 + snap.progress as f64,
                        gap: c.gap,
                        position: c.position,
                    })
                })
                .collect();
            let is_player = idx == history.player_car_idx;
            let name = name_by_idx.get(&idx).cloned().unwrap_or_else(|| {
                if is_player {
                    "Você".to_string()
                } else {
                    format!("Carro {idx}")
                }
            });
            ChartCar {
                idx,
                name,
                is_player,
                points,
            }
        })
        .collect();

    let lap_times: Vec<ChartLapTime> = history
        .player_laps
        .iter()
        .filter(|l| l.time > 0.0)
        .map(|l| ChartLapTime {
            lap: l.lap,
            time_s: l.time,
        })
        .collect();

    // Tempos de volta de todos os carros (para comparar ritmo entre pilotos).
    let car_lap_times: Vec<ChartCarLapTime> = history
        .car_laps
        .iter()
        .filter(|l| l.time > 0.0)
        .map(|l| ChartCarLapTime {
            idx: l.car_idx,
            lap: l.lap,
            time_s: l.time,
        })
        .collect();

    // Gap ao rival por volta (assinado). Última amostra da volta com o rival
    // adjacente vence.
    let (rival_gap, rival_name) = if let Some((ridx, _, _)) = find_rival(history) {
        let mut by_lap: std::collections::BTreeMap<i32, f64> = std::collections::BTreeMap::new();
        for p in &history.player_track {
            if p.ahead_idx == ridx && p.gap_ahead.is_finite() {
                by_lap.insert(p.lap, p.gap_ahead);
            } else if p.behind_idx == ridx && p.gap_behind.is_finite() {
                by_lap.insert(p.lap, -p.gap_behind);
            }
        }
        let v: Vec<ChartGap> = by_lap
            .into_iter()
            .map(|(lap, g)| ChartGap { lap, gap_s: g })
            .collect();
        let name = name_by_idx.get(&ridx).cloned().unwrap_or_default();
        (v, name)
    } else {
        (Vec::new(), String::new())
    };

    if cars.is_empty() && lap_times.is_empty() {
        return None;
    }
    Some(RaceCharts {
        cars,
        yellow_laps: history.yellow_laps.clone(),
        lap_times,
        car_lap_times,
        rival_gap,
        rival_name,
    })
}

/// Peso de cada posição perdida no score do erro (em "segundos equivalentes").
const POS_WEIGHT: f64 = 3.0;
/// Bônus de score quando a volta teve incidente/contato.
const INCIDENT_BONUS: f64 = 2.0;
/// Score mínimo para o erro virar card (corrida limpa não mostra nada).
const MISTAKE_MIN_SCORE: f64 = 3.0;
/// Excesso de tempo (s) que conta como sinal de "volta lenta".
const SLOW_SIGNAL_S: f64 = 1.5;

/// Erro mais caro (2b-2). Pontua cada volta por tempo perdido vs ritmo limpo +
/// posições perdidas + incidente, e escolhe a de maior custo. Confiança baixa é
/// escondida (melhor não mostrar do que inventar drama). DNF domina o card.
fn analyze_mistake(
    history: &RaceHistory,
    incidents: &PlayerIncidents,
    pace: Option<&PaceAnalysis>,
) -> Option<CostlyMistake> {
    // O abandono é, por definição, o momento mais caro da corrida.
    if incidents.is_dnf {
        return Some(CostlyMistake {
            lap: incidents.dnf_lap.unwrap_or(0),
            kind: "dnf".to_string(),
            time_lost_ms: 0.0,
            positions_lost: 0,
            confidence: "alta".to_string(),
        });
    }

    let clean = pace.map(|p| p.clean_avg_ms).unwrap_or(0.0);
    let yellow: std::collections::HashSet<i32> = history.yellow_laps.iter().copied().collect();
    let crash: std::collections::HashSet<i32> = incidents.crash_laps.iter().copied().collect();

    // Volta de largada: o campo inteiro larga junto e acelera do grid, então ela é
    // SEMPRE ~vários segundos mais lenta que o ritmo — sistêmico, não erro do piloto.
    // (O ritmo limpo já a ignora via CLEAN_LAP_FACTOR; aqui neutralizamos só o sinal
    // de RITMO dela pra não virar o "erro mais caro" falso.) Batida/perda de posição
    // na largada continuam contando — um incidente de 1ª volta ainda é flagrado.
    let opening_lap = history
        .player_laps
        .iter()
        .map(|l| l.lap)
        .filter(|lap| *lap > 0)
        .min();

    // Posição representativa por volta = última amostra daquela volta.
    let mut pos_by_lap: HashMap<i32, i32> = HashMap::new();
    for p in &history.player_track {
        if p.position > 0 && p.lap >= 0 {
            pos_by_lap.insert(p.lap, p.position);
        }
    }

    let mut best: Option<(f64, CostlyMistake)> = None;
    for l in &history.player_laps {
        let lap = l.lap;
        // Voltas sob amarela são naturalmente lentas — não punir.
        if lap <= 0 || yellow.contains(&lap) {
            continue;
        }
        let lap_ms = l.time * 1000.0;
        // Na largada, a lentidão é do procedimento (grid parado/pack), não erro —
        // zera o excesso de tempo dela. Perda de posição/batida abaixo ainda valem.
        let is_opening = Some(lap) == opening_lap;
        let slow_excess = if clean > 0.0 && lap_ms > 0.0 && !is_opening {
            (lap_ms - clean).max(0.0)
        } else {
            0.0
        };
        let positions_lost = match (pos_by_lap.get(&lap), pos_by_lap.get(&(lap - 1))) {
            (Some(cur), Some(prev)) => (cur - prev).max(0),
            _ => 0,
        };
        let is_incident = crash.contains(&lap);

        let slow_s = slow_excess / 1000.0;
        let score = slow_s + positions_lost as f64 * POS_WEIGHT
            + if is_incident { INCIDENT_BONUS } else { 0.0 };

        // Quantos sinais BATEM nessa volta → confiança.
        let sig_slow = slow_s >= SLOW_SIGNAL_S;
        let sig_pos = positions_lost >= 1;
        let n = sig_slow as i32 + sig_pos as i32 + is_incident as i32;
        let confidence = if n >= 3 {
            "alta"
        } else if n == 2 {
            "media"
        } else if is_incident || slow_s >= 3.0 || positions_lost >= 2 {
            // Um sinal forte sozinho ainda vale (média).
            "media"
        } else {
            "baixa"
        };
        if confidence == "baixa" {
            continue;
        }

        let kind = if is_incident {
            "incident"
        } else if positions_lost >= 1 && slow_s < 2.0 {
            "position_loss"
        } else {
            "pace_drop"
        };

        let cand = CostlyMistake {
            lap,
            kind: kind.to_string(),
            time_lost_ms: slow_excess,
            positions_lost,
            confidence: confidence.to_string(),
        };
        if best.as_ref().map(|(s, _)| score > *s).unwrap_or(true) {
            best = Some((score, cand));
        }
    }

    // Só mostra se foi minimamente relevante — mas qualquer contato vale o card.
    best.filter(|(s, m)| *s >= MISTAKE_MIN_SCORE || m.kind == "incident")
        .map(|(_, m)| m)
}

/// Score mínimo para o melhor momento virar card (sem destaque → nada).
const BEST_MIN_SCORE: f64 = 3.0;

/// Melhor momento (2b-3) — espelho positivo do erro mais caro. Detecta vários
/// candidatos e escolhe por PRIORIDADE (não só score): ganho de posição > rival
/// superado > recuperação pós-erro > sequência limpa > melhor volta (fallback).
/// Cada candidato tem seu próprio portão de confiança; baixa nunca vira card.
fn analyze_best_moment(
    history: &RaceHistory,
    name_by_idx: &HashMap<i32, String>,
    pace: Option<&PaceAnalysis>,
    incidents: &PlayerIncidents,
    mistake: Option<&CostlyMistake>,
) -> Option<BestMoment> {
    let clean = pace.map(|p| p.clean_avg_ms).unwrap_or(0.0);
    let yellow: std::collections::HashSet<i32> = history.yellow_laps.iter().copied().collect();
    let crash: std::collections::HashSet<i32> = incidents.crash_laps.iter().copied().collect();

    let mut pos_by_lap: HashMap<i32, i32> = HashMap::new();
    for p in &history.player_track {
        if p.position > 0 && p.lap >= 0 {
            pos_by_lap.insert(p.lap, p.position);
        }
    }
    // Volta da melhor marca pessoal.
    let best_lap_lap = history
        .player_laps
        .iter()
        .filter(|l| l.time > 0.0)
        .min_by(|a, b| a.time.partial_cmp(&b.time).unwrap_or(std::cmp::Ordering::Equal))
        .map(|l| l.lap);

    // ── 1. Ganho de posição (prioridade máxima) ─────────────────────────────
    {
        let mut best: Option<(f64, i32, i32, f64)> = None; // score, lap, gain, time_gain
        for l in &history.player_laps {
            let lap = l.lap;
            if lap <= 0 || yellow.contains(&lap) || crash.contains(&lap) {
                continue;
            }
            let lap_ms = l.time * 1000.0;
            // Ataque de verdade não é volta lenta (ganho lento = herdado, não mérito).
            if clean > 0.0 && lap_ms > clean * 1.05 {
                continue;
            }
            let gain = match (pos_by_lap.get(&lap), pos_by_lap.get(&(lap - 1))) {
                (Some(cur), Some(prev)) => (prev - cur).max(0),
                _ => 0,
            };
            if gain < 1 {
                continue;
            }
            let time_gain = if clean > 0.0 && lap_ms > 0.0 {
                (clean - lap_ms).max(0.0)
            } else {
                0.0
            };
            let score = gain as f64 * POS_WEIGHT + time_gain / 1000.0;
            if best.map(|(s, ..)| score > s).unwrap_or(true) {
                best = Some((score, lap, gain, time_gain));
            }
        }
        if let Some((score, lap, gain, time_gain)) = best {
            let is_best = best_lap_lap == Some(lap);
            let n = (gain >= 1) as i32 + (time_gain >= 800.0) as i32 + is_best as i32;
            let conf = if n >= 2 {
                Some("alta")
            } else if gain >= 2 || is_best {
                Some("media")
            } else {
                None // +1 isolado não é "ganho relevante"
            };
            if let Some(conf) = conf {
                if score >= BEST_MIN_SCORE {
                    return Some(BestMoment {
                        lap,
                        kind: "position_gain".to_string(),
                        positions_gained: gain,
                        time_gain_ms: time_gain,
                        streak: 0,
                        rival_name: String::new(),
                        confidence: conf.to_string(),
                    });
                }
            }
        }
    }

    // ── 2. Rival superado ───────────────────────────────────────────────────
    if let Some((ridx, rlaps, _)) = find_rival(history) {
        if rival_beaten(history, ridx) {
            if let Some(name) = name_by_idx.get(&ridx) {
                let conf = if rlaps >= 6 { "alta" } else { "media" };
                return Some(BestMoment {
                    lap: 0,
                    kind: "rival_beaten".to_string(),
                    positions_gained: 0,
                    time_gain_ms: 0.0,
                    streak: rlaps,
                    rival_name: name.clone(),
                    confidence: conf.to_string(),
                });
            }
        }
    }

    // ── 3. Recuperação pós-erro ─────────────────────────────────────────────
    if let Some(m) = mistake {
        if m.kind != "dnf" && m.lap > 0 {
            let pos_at = pos_by_lap.get(&m.lap).copied();
            let regained = pos_at
                .and_then(|pa| {
                    history
                        .player_track
                        .iter()
                        .filter(|p| p.lap > m.lap && p.position > 0)
                        .map(|p| p.position)
                        .min()
                        .map(|best_after| (pa - best_after).max(0))
                })
                .unwrap_or(0);
            let pace_back = clean > 0.0
                && history
                    .player_laps
                    .iter()
                    .any(|l| l.lap > m.lap && l.time > 0.0 && l.time * 1000.0 <= clean * 1.02);
            let signals = (regained >= 1) as i32 + pace_back as i32;
            let score = 2.0 + regained as f64 * 1.5 + if pace_back { 1.0 } else { 0.0 };
            if signals >= 1 && score >= BEST_MIN_SCORE {
                let conf = if regained >= 2 && pace_back { "alta" } else { "media" };
                return Some(BestMoment {
                    lap: 0,
                    kind: "recovery".to_string(),
                    positions_gained: regained,
                    time_gain_ms: 0.0,
                    streak: 0,
                    rival_name: String::new(),
                    confidence: conf.to_string(),
                });
            }
        }
    }

    // ── 4. Sequência limpa ──────────────────────────────────────────────────
    if clean > 0.0 {
        let mut laps: Vec<&PlayerLap> = history.player_laps.iter().filter(|l| l.lap > 0).collect();
        laps.sort_by_key(|l| l.lap);
        let mut best_streak = 0;
        let mut cur = 0;
        let mut prev_lap: Option<i32> = None;
        for l in laps {
            let good = l.time > 0.0
                && l.time * 1000.0 <= clean * CLEAN_LAP_FACTOR
                && !yellow.contains(&l.lap)
                && !crash.contains(&l.lap);
            let consecutive = prev_lap.map(|p| l.lap == p + 1).unwrap_or(true);
            cur = if good && consecutive {
                cur + 1
            } else if good {
                1
            } else {
                0
            };
            best_streak = best_streak.max(cur);
            prev_lap = Some(l.lap);
        }
        if best_streak >= 3 {
            let score = 1.0 + best_streak as f64 * 0.7;
            if score >= BEST_MIN_SCORE {
                let conf = if best_streak >= 5 { "alta" } else { "media" };
                return Some(BestMoment {
                    lap: 0,
                    kind: "clean_streak".to_string(),
                    positions_gained: 0,
                    time_gain_ms: 0.0,
                    streak: best_streak,
                    rival_name: String::new(),
                    confidence: conf.to_string(),
                });
            }
        }
    }

    // ── 5. Melhor volta (fallback bom, não prioridade) ──────────────────────
    if let (Some(bl_lap), Some(p)) = (best_lap_lap, pace) {
        if clean > 0.0 && !yellow.contains(&bl_lap) && !crash.contains(&bl_lap) {
            let margin = clean - p.best_lap_ms; // quão melhor que o ritmo limpo
            if margin >= 800.0 {
                return Some(BestMoment {
                    lap: bl_lap,
                    kind: "best_lap".to_string(),
                    positions_gained: 0,
                    time_gain_ms: margin,
                    streak: 0,
                    rival_name: String::new(),
                    confidence: "media".to_string(),
                });
            }
        }
    }

    None
}

/// Conta os movimentos brutos de posição na trajetória do jogador. Posição MENOR
/// = subiu. Inclui ganhos herdados (alguém à frente abandona também sobe sua
/// posição), por isso é só ESTIMATIVA — o split fino fica com a tabela oficial.
fn analyze_position_flow(history: &RaceHistory) -> Option<PositionFlow> {
    let positions: Vec<i32> = history
        .player_track
        .iter()
        .map(|p| p.position)
        .filter(|p| *p > 0)
        .collect();
    if positions.len() < 3 {
        return None;
    }
    let mut gained = 0;
    let mut lost = 0;
    let mut prev = positions[0];
    for &pos in &positions[1..] {
        if pos < prev {
            gained += prev - pos;
        } else if pos > prev {
            lost += pos - prev;
        }
        prev = pos;
    }
    Some(PositionFlow {
        gained_on_track: gained,
        lost_on_track: lost,
        samples: positions.len() as i32,
    })
}

/// Confiança da análise pela cobertura (voltas do jogador vs corrida).
/// Quando não sabemos a duração da corrida, caímos no número absoluto de voltas.
fn confidence_label(laps_seen: i32, race_laps: i32) -> (String, bool) {
    if race_laps > 0 {
        let coverage = laps_seen as f64 / race_laps as f64;
        let conf = if coverage >= 0.9 {
            "alta"
        } else if coverage >= 0.6 {
            "media"
        } else {
            "baixa"
        };
        // Saiu bem antes do fim (faltaram >= 2 voltas).
        let partial = (race_laps - laps_seen) >= 2;
        (conf.to_string(), partial)
    } else {
        let conf = if laps_seen >= 8 {
            "alta"
        } else if laps_seen >= 4 {
            "media"
        } else {
            "baixa"
        };
        (conf.to_string(), false)
    }
}

fn analyze_pace(history: &RaceHistory, player_idx: i32) -> Option<PaceAnalysis> {
    // Tempos do jogador (segundos → ms).
    let times: Vec<f64> = history
        .player_laps
        .iter()
        .map(|l| l.time)
        .filter(|t| *t > 0.0)
        .map(|t| t * 1000.0)
        .collect();
    // Precisa de um mínimo de voltas válidas para o card de ritmo ter sentido.
    if (times.len() as i32) < MIN_PACE_LAPS {
        return None;
    }
    let best = times.iter().cloned().fold(f64::INFINITY, f64::min);
    let real_avg = mean(&times);
    let clean: Vec<f64> = times
        .iter()
        .cloned()
        .filter(|t| *t <= best * CLEAN_LAP_FACTOR)
        .collect();
    let clean_avg = if clean.is_empty() { real_avg } else { mean(&clean) };

    // Ritmo do campo (todos os carros menos o jogador/pace), em ms.
    let grid_times: Vec<f64> = history
        .car_laps
        .iter()
        .filter(|l| l.car_idx != player_idx && l.time > 0.0)
        .map(|l| l.time * 1000.0)
        .collect();
    let grid_sample = grid_times.len() as i32;
    let grid_avg = mean(&grid_times);
    let vs_grid_reliable = grid_sample >= MIN_GRID_SAMPLE && grid_avg > 0.0;

    Some(PaceAnalysis {
        best_lap_ms: best,
        real_avg_ms: real_avg,
        clean_avg_ms: clean_avg,
        lost_per_lap_ms: (real_avg - clean_avg).max(0.0),
        grid_avg_ms: grid_avg,
        vs_grid_ms: if grid_avg > 0.0 {
            clean_avg - grid_avg
        } else {
            0.0
        },
        good_laps: clean.len() as i32,
        total_laps: times.len() as i32,
        consistency_reliable: (times.len() as i32) >= MIN_CONSISTENCY_LAPS,
        grid_sample,
        vs_grid_reliable,
    })
}

/// Acha o rival (car_idx, voltas ao lado, gap médio) com as mesmas regras
/// anti-falso-positivo do card. Compartilhado pelo card e pelo "melhor momento".
fn find_rival(history: &RaceHistory) -> Option<(i32, i32, f64)> {
    if history.player_track.is_empty() {
        return None;
    }
    // Para cada vizinho (à frente/atrás), junta as voltas vistas e os gaps.
    let mut laps_by_idx: HashMap<i32, std::collections::HashSet<i32>> = HashMap::new();
    let mut gaps_by_idx: HashMap<i32, Vec<f64>> = HashMap::new();
    for p in &history.player_track {
        for (idx, gap) in [(p.ahead_idx, p.gap_ahead), (p.behind_idx, p.gap_behind)] {
            if idx < 0 {
                continue;
            }
            laps_by_idx.entry(idx).or_default().insert(p.lap.max(0));
            if gap.is_finite() && gap >= 0.0 {
                gaps_by_idx.entry(idx).or_default().push(gap);
            }
        }
    }
    // Rival = quem apareceu em MAIS voltas ao seu lado.
    let (rival_idx, laps) = laps_by_idx
        .iter()
        .map(|(idx, laps)| (*idx, laps.len() as i32))
        .max_by_key(|(_, n)| *n)?;
    // Anti-falso-positivo: só é "rival" com disputa real — voltas suficientes
    // ao lado E gap médio pequeno. Caso contrário, sem rival claro (None).
    if laps < MIN_RIVAL_LAPS {
        return None;
    }
    let gaps = gaps_by_idx.get(&rival_idx)?;
    if gaps.is_empty() {
        return None;
    }
    let avg_gap = mean(gaps);
    if avg_gap > MAX_RIVAL_GAP_S {
        return None;
    }
    Some((rival_idx, laps, avg_gap))
}

fn analyze_rival(history: &RaceHistory, name_by_idx: &HashMap<i32, String>) -> Option<RivalCard> {
    let (rival_idx, laps, avg_gap) = find_rival(history)?;
    let name = name_by_idx.get(&rival_idx)?.clone();
    Some(RivalCard {
        pilot_name: name,
        laps_battled: laps,
        avg_gap_s: avg_gap,
    })
}

/// Você terminou À FRENTE do rival? Última adjacência vence: se na última vez que
/// ele apareceu ao seu lado estava ATRÁS (behind_idx), você venceu a disputa.
fn rival_beaten(history: &RaceHistory, rival_idx: i32) -> bool {
    let mut beaten = None;
    for p in &history.player_track {
        if p.ahead_idx == rival_idx {
            beaten = Some(false);
        } else if p.behind_idx == rival_idx {
            beaten = Some(true);
        }
    }
    beaten.unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::iracing_sdk::race_monitor::{
        CarLap, LapSnapshot, PlayerLap, PlayerTrackPoint, RaceHistory,
    };

    fn base_history() -> RaceHistory {
        let mut h = RaceHistory::clone(&empty());
        h.player_car_idx = 0;
        h
    }
    // RaceHistory não expõe Default; montamos pelo serde de um JSON mínimo.
    fn empty() -> RaceHistory {
        serde_json::from_value(serde_json::json!({
            "laps": [], "player_laps": [], "player_track": [], "yellow_laps": [],
            "player_car_idx": 0, "attempt_number": 1, "finished": true, "outcome": "Finalizada",
            "car_laps": [], "cars_meta": [], "track_id": 1, "qualy_laps": []
        }))
        .unwrap()
    }

    #[test]
    fn pace_e_consistencia() {
        let mut h = base_history();
        // Voltas do jogador: 90, 90.5, 91, e uma ruim 96 (erro).
        h.player_laps = vec![
            PlayerLap { lap: 1, time: 90.0, fuel_remaining: -1.0 },
            PlayerLap { lap: 2, time: 90.5, fuel_remaining: -1.0 },
            PlayerLap { lap: 3, time: 91.0, fuel_remaining: -1.0 },
            PlayerLap { lap: 4, time: 96.0, fuel_remaining: -1.0 },
        ];
        // Campo um pouco mais lento.
        for lap in 1..=4 {
            h.car_laps.push(CarLap { car_idx: 1, lap, time: 92.0 });
        }
        let a = analyze(&h, &HashMap::new(), &HashMap::new(), &PlayerIncidents::default());
        let p = a.pace.expect("tem ritmo");
        assert!((p.best_lap_ms - 90_000.0).abs() < 1.0);
        // Volta limpa exclui a de 96 (96000 > 90000*1.04=93600).
        assert_eq!(p.good_laps, 3);
        assert_eq!(p.total_laps, 4);
        // Perdeu tempo por causa da volta ruim.
        assert!(p.lost_per_lap_ms > 0.0);
        // Mais rápido que o campo (limpo ~90.5s vs campo 92s).
        assert!(p.vs_grid_ms < 0.0);
        // 4 voltas → consistência confiável; 4 voltas do campo → vs_grid confiável.
        assert!(p.consistency_reliable);
        assert!(p.vs_grid_reliable);
        assert_eq!(p.grid_sample, 4);
    }

    #[test]
    fn ritmo_some_mas_consistencia_nao_confiavel_com_2_voltas() {
        let mut h = base_history();
        h.player_laps = vec![
            PlayerLap { lap: 1, time: 90.0, fuel_remaining: -1.0 },
            PlayerLap { lap: 2, time: 90.5, fuel_remaining: -1.0 },
        ];
        let a = analyze(&h, &HashMap::new(), &HashMap::new(), &PlayerIncidents::default());
        let p = a.pace.expect("2 voltas já dá ritmo");
        assert_eq!(p.total_laps, 2);
        // < 3 voltas → consistência não confiável (a tela esconde o card).
        assert!(!p.consistency_reliable);
        // sem voltas do campo → vs_grid não confiável.
        assert!(!p.vs_grid_reliable);
    }

    #[test]
    fn uma_volta_so_nao_gera_ritmo() {
        let mut h = base_history();
        h.player_laps = vec![PlayerLap { lap: 1, time: 90.0, fuel_remaining: -1.0 }];
        let a = analyze(&h, &HashMap::new(), &HashMap::new(), &PlayerIncidents::default());
        assert!(a.pace.is_none());
    }

    #[test]
    fn rival_e_quem_mais_brigou() {
        let mut h = base_history();
        // O carro idx 5 fica à frente/atrás por 3 voltas; o 9 só 1 volta.
        h.player_track = vec![
            PlayerTrackPoint { session_time: 1.0, lap: 1, position: 5, speed_kmh: 200.0, ahead_idx: 5, gap_ahead: 0.5, behind_idx: 9, gap_behind: 0.8 },
            PlayerTrackPoint { session_time: 2.0, lap: 2, position: 5, speed_kmh: 200.0, ahead_idx: 5, gap_ahead: 0.7, behind_idx: -1, gap_behind: 0.0 },
            PlayerTrackPoint { session_time: 3.0, lap: 3, position: 5, speed_kmh: 200.0, ahead_idx: 5, gap_ahead: 0.6, behind_idx: -1, gap_behind: 0.0 },
        ];
        let mut names = HashMap::new();
        names.insert(5, "Lucas Silva".to_string());
        names.insert(9, "Rafael Costa".to_string());
        let a = analyze(&h, &names, &HashMap::new(), &PlayerIncidents::default());
        let r = a.rival.expect("tem rival");
        assert_eq!(r.pilot_name, "Lucas Silva");
        assert_eq!(r.laps_battled, 3);
        assert!((r.avg_gap_s - 0.6).abs() < 0.05);
    }

    #[test]
    fn rival_rejeitado_com_poucas_voltas() {
        let mut h = base_history();
        // Só 2 voltas ao lado do idx 5 — abaixo do mínimo de rival.
        h.player_track = vec![
            PlayerTrackPoint { session_time: 1.0, lap: 1, position: 5, speed_kmh: 200.0, ahead_idx: 5, gap_ahead: 0.5, behind_idx: -1, gap_behind: 0.0 },
            PlayerTrackPoint { session_time: 2.0, lap: 2, position: 5, speed_kmh: 200.0, ahead_idx: 5, gap_ahead: 0.6, behind_idx: -1, gap_behind: 0.0 },
        ];
        let mut names = HashMap::new();
        names.insert(5, "Lucas Silva".to_string());
        let a = analyze(&h, &names, &HashMap::new(), &PlayerIncidents::default());
        assert!(a.rival.is_none(), "2 voltas não é rival");
    }

    #[test]
    fn rival_rejeitado_com_gap_grande() {
        let mut h = base_history();
        // 3 voltas, mas sempre muito longe (gap > 3s) — não é disputa.
        h.player_track = vec![
            PlayerTrackPoint { session_time: 1.0, lap: 1, position: 5, speed_kmh: 200.0, ahead_idx: 5, gap_ahead: 8.0, behind_idx: -1, gap_behind: 0.0 },
            PlayerTrackPoint { session_time: 2.0, lap: 2, position: 5, speed_kmh: 200.0, ahead_idx: 5, gap_ahead: 9.0, behind_idx: -1, gap_behind: 0.0 },
            PlayerTrackPoint { session_time: 3.0, lap: 3, position: 5, speed_kmh: 200.0, ahead_idx: 5, gap_ahead: 7.5, behind_idx: -1, gap_behind: 0.0 },
        ];
        let mut names = HashMap::new();
        names.insert(5, "Lucas Silva".to_string());
        let a = analyze(&h, &names, &HashMap::new(), &PlayerIncidents::default());
        assert!(a.rival.is_none(), "gap grande não é rival");
    }

    #[test]
    fn confianca_alta_quando_cobre_a_corrida_toda() {
        let mut h = base_history();
        // Corrida de 10 voltas (líder), jogador fez 10.
        for lap in 1..=10 {
            h.laps.push(LapSnapshot { lap, progress: 0.0, cars: vec![] });
            h.player_laps.push(PlayerLap { lap, time: 90.0, fuel_remaining: -1.0 });
        }
        let a = analyze(&h, &HashMap::new(), &HashMap::new(), &PlayerIncidents::default());
        assert_eq!(a.race_laps, 10);
        assert_eq!(a.laps_seen, 10);
        assert_eq!(a.confidence, "alta");
        assert!(!a.is_partial);
    }

    #[test]
    fn confianca_baixa_e_parcial_quando_saiu_cedo() {
        let mut h = base_history();
        // Corrida de 12 voltas, jogador fez só 3 → saiu cedo.
        for lap in 1..=12 {
            h.laps.push(LapSnapshot { lap, progress: 0.0, cars: vec![] });
        }
        for lap in 1..=3 {
            h.player_laps.push(PlayerLap { lap, time: 90.0, fuel_remaining: -1.0 });
        }
        let a = analyze(&h, &HashMap::new(), &HashMap::new(), &PlayerIncidents::default());
        assert_eq!(a.race_laps, 12);
        assert_eq!(a.last_lap_seen, 3);
        assert_eq!(a.confidence, "baixa");
        assert!(a.is_partial);
    }

    #[test]
    fn position_flow_conta_subidas_e_quedas() {
        let mut h = base_history();
        // Trajetória de posição: P14 → P12 (subiu 2) → P13 (caiu 1) → P8 (subiu 5).
        let pos_seq = [14, 14, 12, 12, 13, 8, 8];
        h.player_track = pos_seq
            .iter()
            .enumerate()
            .map(|(i, &pos)| PlayerTrackPoint {
                session_time: i as f64,
                lap: i as i32,
                position: pos,
                speed_kmh: 200.0,
                ahead_idx: -1,
                gap_ahead: 0.0,
                behind_idx: -1,
                gap_behind: 0.0,
            })
            .collect();
        let a = analyze(&h, &HashMap::new(), &HashMap::new(), &PlayerIncidents::default());
        let f = a.position_flow.expect("tem fluxo de posição");
        assert_eq!(f.gained_on_track, 7); // 2 + 5
        assert_eq!(f.lost_on_track, 1);
        assert_eq!(f.samples, 7);
    }

    #[test]
    fn erro_mais_caro_incidente_com_perda() {
        let mut h = base_history();
        // Ritmo limpo ~90s; volta 7 explode para 95s (perdeu ~5s).
        h.player_laps = vec![
            PlayerLap { lap: 5, time: 90.0, fuel_remaining: -1.0 },
            PlayerLap { lap: 6, time: 90.0, fuel_remaining: -1.0 },
            PlayerLap { lap: 7, time: 95.0, fuel_remaining: -1.0 },
            PlayerLap { lap: 8, time: 90.0, fuel_remaining: -1.0 },
        ];
        // Caiu de P7 (fim da volta 6) para P9 (fim da volta 7).
        h.player_track = vec![
            PlayerTrackPoint { session_time: 6.0, lap: 6, position: 7, speed_kmh: 200.0, ahead_idx: -1, gap_ahead: 0.0, behind_idx: -1, gap_behind: 0.0 },
            PlayerTrackPoint { session_time: 7.0, lap: 7, position: 9, speed_kmh: 150.0, ahead_idx: -1, gap_ahead: 0.0, behind_idx: -1, gap_behind: 0.0 },
        ];
        let inc = PlayerIncidents { crash_laps: vec![7], is_dnf: false, dnf_lap: None };
        let a = analyze(&h, &HashMap::new(), &HashMap::new(), &inc);
        let m = a.mistake.expect("tem erro mais caro");
        assert_eq!(m.lap, 7);
        assert_eq!(m.kind, "incident");
        assert_eq!(m.positions_lost, 2);
        assert!(m.time_lost_ms > 3000.0);
        assert_eq!(m.confidence, "alta"); // lenta + perda + incidente
    }

    #[test]
    fn erro_mais_caro_dnf_domina() {
        let mut h = base_history();
        h.player_laps = vec![PlayerLap { lap: 1, time: 90.0, fuel_remaining: -1.0 }, PlayerLap { lap: 2, time: 90.0, fuel_remaining: -1.0 }];
        let inc = PlayerIncidents { crash_laps: vec![], is_dnf: true, dnf_lap: Some(9) };
        let a = analyze(&h, &HashMap::new(), &HashMap::new(), &inc);
        let m = a.mistake.expect("DNF é o erro mais caro");
        assert_eq!(m.kind, "dnf");
        assert_eq!(m.lap, 9);
    }

    #[test]
    fn corrida_limpa_nao_mostra_erro() {
        let mut h = base_history();
        // Voltas consistentes, sem incidente nem perda de posição.
        h.player_laps = (1..=8)
            .map(|lap| PlayerLap { lap, time: 90.0 + (lap as f64 % 2.0) * 0.2, fuel_remaining: -1.0 })
            .collect();
        let a = analyze(&h, &HashMap::new(), &HashMap::new(), &PlayerIncidents::default());
        assert!(a.mistake.is_none(), "corrida limpa não inventa erro");
    }

    #[test]
    fn largada_lenta_nao_vira_erro_mais_caro() {
        let mut h = base_history();
        // Volta de largada (2) larga do grid: +10s vs ritmo. O erro REAL é na 13
        // (+4s). Sem a proteção, a largada (+10s) domina; com ela, o erro certo aparece.
        let mut laps = vec![PlayerLap { lap: 2, time: 100.0, fuel_remaining: -1.0 }];
        for lap in 3..=12 {
            laps.push(PlayerLap { lap, time: 90.0, fuel_remaining: -1.0 });
        }
        laps.push(PlayerLap { lap: 13, time: 94.0, fuel_remaining: -1.0 });
        h.player_laps = laps;
        let a = analyze(&h, &HashMap::new(), &HashMap::new(), &PlayerIncidents::default());
        let m = a.mistake.expect("o erro da volta 13 ainda é um erro");
        assert_eq!(m.lap, 13, "a largada não pode roubar o erro mais caro");
        assert_eq!(m.kind, "pace_drop");
    }

    #[test]
    fn batida_na_largada_ainda_conta() {
        let mut h = base_history();
        // Largada lenta E com batida: o incidente na largada continua flagrado
        // (só a lentidão sistêmica é neutralizada, não o contato real).
        let mut laps = vec![PlayerLap { lap: 2, time: 100.0, fuel_remaining: -1.0 }];
        for lap in 3..=8 {
            laps.push(PlayerLap { lap, time: 90.0, fuel_remaining: -1.0 });
        }
        h.player_laps = laps;
        let inc = PlayerIncidents { crash_laps: vec![2], is_dnf: false, dnf_lap: None };
        let a = analyze(&h, &HashMap::new(), &HashMap::new(), &inc);
        let m = a.mistake.expect("batida na largada é um incidente");
        assert_eq!(m.lap, 2);
        assert_eq!(m.kind, "incident");
    }

    #[test]
    fn melhor_momento_ataque_decisivo() {
        let mut h = base_history();
        // Ritmo ~90s; volta 2 é a melhor (89s) e ganhou 2 posições.
        h.player_laps = vec![
            PlayerLap { lap: 1, time: 90.0, fuel_remaining: -1.0 },
            PlayerLap { lap: 2, time: 89.0, fuel_remaining: -1.0 },
            PlayerLap { lap: 3, time: 90.0, fuel_remaining: -1.0 },
            PlayerLap { lap: 4, time: 90.0, fuel_remaining: -1.0 },
        ];
        // P10 no fim da volta 1 → P8 no fim da volta 2 (ganho de 2).
        h.player_track = vec![
            PlayerTrackPoint { session_time: 1.0, lap: 1, position: 10, speed_kmh: 200.0, ahead_idx: -1, gap_ahead: 0.0, behind_idx: -1, gap_behind: 0.0 },
            PlayerTrackPoint { session_time: 2.0, lap: 2, position: 8, speed_kmh: 205.0, ahead_idx: -1, gap_ahead: 0.0, behind_idx: -1, gap_behind: 0.0 },
        ];
        let a = analyze(&h, &HashMap::new(), &HashMap::new(), &PlayerIncidents::default());
        let b = a.best_moment.expect("tem melhor momento");
        assert_eq!(b.kind, "position_gain");
        assert_eq!(b.lap, 2);
        assert_eq!(b.positions_gained, 2);
        assert_eq!(b.confidence, "alta"); // melhor volta + ganho
    }

    #[test]
    fn melhor_momento_rival_superado() {
        let mut h = base_history();
        // Sem voltas registradas (foco no rival); idx 7 fica ATRÁS por 6 voltas.
        h.player_track = (1..=6)
            .map(|lap| PlayerTrackPoint {
                session_time: lap as f64,
                lap,
                position: 5,
                speed_kmh: 200.0,
                ahead_idx: -1,
                gap_ahead: 0.0,
                behind_idx: 7,
                gap_behind: 0.5,
            })
            .collect();
        let mut names = HashMap::new();
        names.insert(7, "Carlos Mendes".to_string());
        let a = analyze(&h, &names, &HashMap::new(), &PlayerIncidents::default());
        let b = a.best_moment.expect("tem melhor momento");
        assert_eq!(b.kind, "rival_beaten");
        assert_eq!(b.rival_name, "Carlos Mendes");
        assert_eq!(b.streak, 6);
        assert_eq!(b.confidence, "alta");
    }

    #[test]
    fn charts_monta_trace_e_tempos() {
        use crate::iracing_sdk::race_monitor::CarGapPoint;
        let mut h = base_history();
        // 2 voltas de trace com 2 carros (jogador idx 0 + idx 1).
        for lap in 1..=2 {
            h.laps.push(LapSnapshot {
                lap,
                progress: 0.0,
                cars: vec![
                    CarGapPoint { idx: 0, position: 3, gap: 1.2, ..Default::default() },
                    CarGapPoint { idx: 1, position: 1, gap: 0.0, ..Default::default() },
                ],
            });
            h.player_laps.push(PlayerLap { lap, time: 90.0, fuel_remaining: -1.0 });
        }
        let mut names = HashMap::new();
        names.insert(1, "Lider Silva".to_string());
        let a = analyze(&h, &names, &HashMap::new(), &PlayerIncidents::default());
        let c = a.charts.expect("tem gráficos");
        assert_eq!(c.cars.len(), 2);
        assert!(c.cars.iter().any(|car| car.is_player && car.points.len() == 2));
        assert_eq!(c.lap_times.len(), 2);
    }

    #[test]
    fn sem_telemetria_nao_quebra() {
        let h = base_history();
        let a = analyze(&h, &HashMap::new(), &HashMap::new(), &PlayerIncidents::default());
        assert!(!a.has_telemetry);
        assert!(a.pace.is_none());
        assert!(a.rival.is_none());
    }

    #[test]
    fn combustivel_calcula_consumo_e_autonomia() {
        let mut h = base_history();
        h.player_laps = vec![
            PlayerLap { lap: 1, time: 90.0, fuel_remaining: 40.0 },
            PlayerLap { lap: 2, time: 90.0, fuel_remaining: 38.0 },
            PlayerLap { lap: 3, time: 90.0, fuel_remaining: 36.0 },
        ];
        let fuel = analyze_fuel(&h).expect("tem combustível");
        assert!((fuel.used_per_lap_l - 2.0).abs() < 1e-6, "2 L/volta");
        assert!((fuel.remaining_l - 36.0).abs() < 1e-6);
        assert!((fuel.laps_left - 18.0).abs() < 1e-6, "36 / 2 = 18 voltas");
    }

    #[test]
    fn combustivel_none_sem_dado() {
        let mut h = base_history();
        h.player_laps = vec![
            PlayerLap { lap: 1, time: 90.0, fuel_remaining: -1.0 },
            PlayerLap { lap: 2, time: 90.0, fuel_remaining: -1.0 },
        ];
        assert!(analyze_fuel(&h).is_none());
    }

    #[test]
    fn setores_apontam_o_ponto_fraco() {
        use crate::iracing_sdk::race_monitor::SectorSplit;
        let mut h = base_history();
        // S2 é o fraco: 30.0 e 31.0 → melhor 30.0, média 30.5, perda 0.5s.
        h.player_sectors = vec![
            SectorSplit { lap: 1, sector: 1, time: 20.0 },
            SectorSplit { lap: 2, sector: 1, time: 20.1 },
            SectorSplit { lap: 1, sector: 2, time: 30.0 },
            SectorSplit { lap: 2, sector: 2, time: 31.0 },
            SectorSplit { lap: 1, sector: 3, time: 25.0 },
            SectorSplit { lap: 2, sector: 3, time: 25.05 },
        ];
        let s = analyze_sectors(&h).expect("tem setores");
        assert_eq!(s.weakest_sector, 2);
        assert!((s.weakest_loss_ms - 500.0).abs() < 1.0, "perda ~0.5s no S2");
        assert!((s.best_ms[1] - 30000.0).abs() < 1.0, "melhor S2 = 30.0s");
    }

    fn track_point(session_time: f64, lap: i32, position: i32, ahead: i32, ga: f64, behind: i32, gb: f64) -> PlayerTrackPoint {
        PlayerTrackPoint {
            session_time,
            lap,
            position,
            speed_kmh: 200.0,
            ahead_idx: ahead,
            gap_ahead: ga,
            behind_idx: behind,
            gap_behind: gb,
        }
    }

    #[test]
    fn dossie_consistencia_maior_para_voltas_parelhas() {
        let mut tight = base_history();
        tight.player_laps = vec![
            PlayerLap { lap: 1, time: 90.0, fuel_remaining: -1.0 },
            PlayerLap { lap: 2, time: 90.1, fuel_remaining: -1.0 },
            PlayerLap { lap: 3, time: 90.0, fuel_remaining: -1.0 },
            PlayerLap { lap: 4, time: 90.2, fuel_remaining: -1.0 },
        ];
        let mut messy = base_history();
        messy.player_laps = vec![
            PlayerLap { lap: 1, time: 90.0, fuel_remaining: -1.0 },
            PlayerLap { lap: 2, time: 90.0, fuel_remaining: -1.0 },
            PlayerLap { lap: 3, time: 90.0, fuel_remaining: -1.0 },
            PlayerLap { lap: 4, time: 97.0, fuel_remaining: -1.0 },
        ];
        let empty_tel = TelemetryAnalysis::default();
        let c_tight = extract_player_race_telemetry(&tight, &empty_tel).unwrap().consistency;
        let c_messy = extract_player_race_telemetry(&messy, &empty_tel).unwrap().consistency;
        assert!(c_tight > c_messy, "parelho={c_tight} vs bagunçado={c_messy}");
        assert!(c_tight > 90.0);
    }

    #[test]
    fn dossie_consistencia_indisponivel_com_poucas_voltas() {
        let mut h = base_history();
        h.player_laps = vec![
            PlayerLap { lap: 1, time: 90.0, fuel_remaining: -1.0 },
            PlayerLap { lap: 2, time: 90.0, fuel_remaining: -1.0 },
        ];
        let row = extract_player_race_telemetry(&h, &TelemetryAnalysis::default()).unwrap();
        assert_eq!(row.consistency, -1.0, "< 3 voltas → não computável");
        assert_eq!(row.laps_seen, 2);
    }

    #[test]
    fn dossie_fracao_de_briga() {
        let mut h = base_history();
        // 12 amostras: 9 com vizinho a < 1s (briga), 3 sozinho longe.
        let mut pts = Vec::new();
        for i in 0..9 {
            pts.push(track_point(i as f64, 1, 5, 3, 0.4, -1, 0.0));
        }
        for i in 9..12 {
            pts.push(track_point(i as f64, 1, 5, 3, 5.0, -1, 0.0));
        }
        h.player_track = pts;
        let row = extract_player_race_telemetry(&h, &TelemetryAnalysis::default()).unwrap();
        assert!((row.battle_fraction - 9.0 / 12.0).abs() < 1e-6, "got {}", row.battle_fraction);
    }

    #[test]
    fn dossie_start_delta_conta_ganho_na_largada() {
        let mut h = base_history();
        // Largou P10 (t=0), em +20s já é P6 → ganhou 4 na largada.
        h.player_track = vec![
            track_point(0.0, 1, 10, -1, 0.0, -1, 0.0),
            track_point(10.0, 1, 8, -1, 0.0, -1, 0.0),
            track_point(21.0, 1, 6, -1, 0.0, -1, 0.0),
            track_point(40.0, 2, 6, -1, 0.0, -1, 0.0),
        ];
        let row = extract_player_race_telemetry(&h, &TelemetryAnalysis::default()).unwrap();
        assert!(row.start_valid);
        assert_eq!(row.start_delta, 4);
    }

    #[test]
    fn dossie_none_sem_nada_utilizavel() {
        let h = base_history();
        assert!(extract_player_race_telemetry(&h, &TelemetryAnalysis::default()).is_none());
    }
}
