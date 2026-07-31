#![allow(dead_code)]
//! Percepção de rivalidade a partir dos dados REAIS do SDK do iRacing. **Puro.**
//!
//! Camada de ENTRADA do sistema de rivalidade: lê o `RaceHistory` de uma corrida
//! disputada e monta, para um CARRO-SONDA qualquer (`probe_car_idx`), o "livro-razão"
//! de interações contra cada oponente — duelo prolongado, pêndulo de posições e
//! ultrapassagem decisiva — e, quando o probe é o jogador, o contato atribuído.
//!
//! Fonte primária = o **trace de campo** (`history.laps`: posição + gap-ao-líder de
//! todos, por volta e a cada troca). Isso funciona para QUALQUER carro — é o que
//! permite calibrar apontando para uma IA sem o jogador precisar dirigir. O
//! `player_track` (~1Hz + contato) é enriquecimento exclusivo do probe-jogador
//! (entra numa fase seguinte; aqui o contato chega pronto via [`ContactSeed`]).
//!
//! IMPORTANTE: esta camada só PERCEBE e PROJETA os deltas — ela NÃO aplica nada no
//! motor de rivalidade. É a base de calibração/debug (o explicador "por que essa
//! rivalidade?"). A aplicação (`apply_rivalry_event` + episódios) vem depois.
//!
//! Ver `docs/superpowers/specs/2026-07-18-track-rivalry-perception-design.md`.

use std::collections::{HashMap, HashSet};

use serde::Serialize;

use super::race_monitor::{CarGapPoint, RaceHistory};

// ── Parâmetros de percepção (CALIBRÁVEIS) ─────────────────────────────────────

/// Limiares e pesos da percepção. Os defaults são o ponto de partida a calibrar
/// com corridas reais gravadas (não há Monte Carlo — é dado real).
#[derive(Clone, Debug, Serialize)]
pub struct PerceptionParams {
    /// Gap (s) entre o par para considerar "colado" (só vale com posições adjacentes).
    pub gap_close_secs: f64,
    /// Voltas coladas mínimas para contar como duelo prolongado.
    pub min_duel_laps: i32,
    /// Voltas coladas que saturam o duelo (topo do crescimento do delta).
    pub duel_saturation_laps: i32,
    /// Trocas de posição mínimas entre o par para contar como pêndulo.
    pub min_swaps: i32,
    /// Trocas que saturam o pêndulo.
    pub swap_saturation: i32,
    /// Última posição considerada "pódio" (relevância alta).
    pub podium_positions: i32,
    /// Última posição considerada "zona de pontos" (relevância média).
    pub points_positions: i32,
    pub relevance_podium: f64,
    pub relevance_points: f64,
    pub relevance_rest: f64,
    /// Fração da corrida a partir da qual uma ultrapassagem é "tarde".
    pub late_race_frac: f64,
    /// Cap agregado por corrida no eixo histórico (freio anti-inflação).
    pub cap_historical: f64,
    /// Cap agregado por corrida no eixo recente.
    pub cap_recent: f64,
}

impl Default for PerceptionParams {
    fn default() -> Self {
        Self {
            gap_close_secs: 1.0,
            min_duel_laps: 3,
            duel_saturation_laps: 10,
            min_swaps: 2,
            swap_saturation: 4,
            podium_positions: 3,
            points_positions: 10,
            relevance_podium: 1.6,
            relevance_points: 1.1,
            relevance_rest: 0.6,
            late_race_frac: 0.75,
            cap_historical: 10.0,
            cap_recent: 22.0,
        }
    }
}

// ── Semente de contato (só quando o probe é o jogador) ────────────────────────

/// Severidade do contato atribuído, mapeada dos tiers de colisão da simulação.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum ContactTier {
    Critical,
    Dnf,
    Major,
    Minor,
}

impl ContactTier {
    /// (historical_delta, recent_delta) — espelha `process_collisions_rivalry`.
    fn deltas(self) -> (f64, f64) {
        match self {
            ContactTier::Critical => (7.0, 18.0),
            ContactTier::Dnf => (5.0, 14.0),
            ContactTier::Major => (3.0, 10.0),
            ContactTier::Minor => (2.0, 8.0),
        }
    }

    fn label(self) -> &'static str {
        match self {
            ContactTier::Critical => "crítico",
            ContactTier::Dnf => "causou DNF",
            ContactTier::Major => "grave",
            ContactTier::Minor => "leve",
        }
    }
}

/// Contato que o jogador sofreu (de `Attempt.collided_with_car_number`), já resolvido
/// ao `car_idx` do culpado. Só existe para o probe-jogador.
#[derive(Clone, Copy, Debug, Serialize)]
pub struct ContactSeed {
    pub opponent_car_idx: i32,
    pub tier: ContactTier,
}

// ── Saída: o livro-razão ──────────────────────────────────────────────────────

/// Um sinal percebido contra um oponente, com o "por quê" e o delta que geraria.
#[derive(Clone, Debug, Serialize)]
pub struct SignalHit {
    /// "contato" | "duelo" | "pendulo" | "ultrapassagem"
    pub kind: String,
    /// Explicação legível (para o explicador de debug).
    pub detail: String,
    pub historical_delta: f64,
    pub recent_delta: f64,
}

/// O que a percepção viu (e projetaria) contra UM oponente numa corrida.
#[derive(Clone, Debug, Serialize)]
pub struct OpponentLedger {
    pub car_idx: i32,
    /// Número do carro (`CarNumberRaw`) — a ponte para o `driver_id` na aplicação.
    pub car_number: i32,
    // ── métricas cruas (o "por quê", sempre reportadas) ──
    /// Voltas distintas coladas e adjacentes.
    pub duel_laps: i32,
    /// Menor gap (s) visto enquanto adjacente. `None` → nunca colou.
    pub closest_gap_secs: Option<f64>,
    /// Trocas de posição entre o par (fora de amarela/box).
    pub swaps: i32,
    /// O probe passou o oponente.
    pub overtakes_for: i32,
    /// O oponente passou o probe.
    pub overtakes_against: i32,
    /// Ultrapassagens (qualquer direção) nas voltas finais.
    pub late_overtakes: i32,
    /// Melhor (menor) posição disputada de perto. 0 = desconhecida.
    pub best_fight_position: i32,
    pub relevance_mult: f64,
    // ── sinais + agregado projetado (NÃO aplicado) ──
    pub hits: Vec<SignalHit>,
    pub historical_delta: f64,
    pub recent_delta: f64,
    /// Intensidade percebida projetada (0.4h + 0.6r) — só informativa.
    pub projected_perceived: f64,
    /// Se o cap por corrida cortou o agregado.
    pub capped: bool,
}

/// Resultado da percepção de uma corrida para um carro-sonda.
#[derive(Clone, Debug, Serialize)]
pub struct RivalryPerception {
    pub probe_car_idx: i32,
    pub is_player_probe: bool,
    pub total_laps: i32,
    /// Oponentes ordenados por intensidade projetada (desc).
    pub opponents: Vec<OpponentLedger>,
    pub params: PerceptionParams,
}

// ── Acumulador interno por oponente ───────────────────────────────────────────

#[derive(Default)]
struct Acc {
    duel_laps: HashSet<i32>,
    closest_gap: f64, // começa em +inf via new()
    swaps: i32,
    overtakes_for: i32,
    overtakes_against: i32,
    late_overtakes: i32,
    best_fight_position: i32, // 0 = desconhecida
    /// sinal de (probe.pos - opp.pos) no último frame visto (0 = ainda não visto).
    prev_sign: i32,
    contact: Option<ContactTier>,
}

impl Acc {
    fn new() -> Self {
        Acc {
            closest_gap: f64::INFINITY,
            ..Default::default()
        }
    }

    fn note_fight_position(&mut self, pos: i32) {
        if pos >= 1 && (self.best_fight_position == 0 || pos < self.best_fight_position) {
            self.best_fight_position = pos;
        }
    }
}

// ── Núcleo ────────────────────────────────────────────────────────────────────

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

/// Tempo de volta assumido quando o `EstTime` não vier (mantém o gate em fração de
/// volta, sem regredir para "tudo colado").
const FALLBACK_LAP_SECS: f64 = 100.0;

/// Gap na pista entre dois carros, em SEGUNDOS, à prova de wrap. Usa a diferença de
/// `LapDistPct` (fração de volta, sempre populada) escalada pelo tempo de volta
/// estimado (`est_time ≈ pct·lap_time` → `lap_time ≈ est/pct` do carro mais
/// adiantado; fallback constante se o `EstTime` não vier — ex.: sessão de IA).
/// NÃO usa `CarIdxF2Time` (gap ao líder, 0/instável fora de multiplayer).
fn track_gap_secs(a: &CarGapPoint, b: &CarGapPoint) -> f64 {
    let mut frac = (a.lap_dist_pct - b.lap_dist_pct).abs() as f64;
    if frac > 0.5 {
        frac = 1.0 - frac; // a pista é circular (0..1)
    }
    let (pct, est) = if a.lap_dist_pct >= b.lap_dist_pct {
        (a.lap_dist_pct as f64, a.est_time as f64)
    } else {
        (b.lap_dist_pct as f64, b.est_time as f64)
    };
    let lap_secs = if pct > 0.15 && est > 5.0 {
        est / pct
    } else {
        FALLBACK_LAP_SECS
    };
    frac * lap_secs
}

/// Percebe as rivalidades de `probe_car_idx` numa corrida. Puro.
pub fn perceive_rivalries(
    history: &RaceHistory,
    probe_car_idx: i32,
    contact: Option<ContactSeed>,
    params: &PerceptionParams,
) -> RivalryPerception {
    let is_player_probe = probe_car_idx == history.player_car_idx;
    let total_laps = history.laps.iter().map(|s| s.lap).max().unwrap_or(0);

    // Máscaras de ruído.
    let yellow: HashSet<i32> = history.yellow_laps.iter().copied().collect();
    // (car_idx, lap) das paradas de box — o gap balança nessas voltas.
    let pit: HashSet<(i32, i32)> = history
        .pit_stops
        .iter()
        .map(|p| (p.car_idx, p.lap))
        .collect();
    let pit_near = |car_idx: i32, lap: i32| -> bool {
        pit.contains(&(car_idx, lap))
            || pit.contains(&(car_idx, lap - 1))
            || pit.contains(&(car_idx, lap + 1))
    };

    // car_idx -> car_number (para a ponte com o driver_id na aplicação).
    let car_number: HashMap<i32, i32> = history
        .cars_meta
        .iter()
        .map(|c| (c.idx, c.car_number))
        .collect();

    let mut accs: HashMap<i32, Acc> = HashMap::new();

    for snap in &history.laps {
        let in_yellow = yellow.contains(&snap.lap);

        // Posição/gap do probe neste snapshot.
        let Some(probe) = snap.cars.iter().find(|c| c.idx == probe_car_idx) else {
            continue;
        };

        for other in &snap.cars {
            if other.idx == probe_car_idx {
                continue;
            }
            let acc = accs.entry(other.idx).or_insert_with(Acc::new);

            let pos_diff = probe.position - other.position; // <0: probe à frente
            let sign = pos_diff.signum();
            let adjacent = pos_diff.abs() == 1;
            let inter_gap = track_gap_secs(probe, other);
            let in_pit = pit_near(probe_car_idx, snap.lap) || pit_near(other.idx, snap.lap);

            // ── Duelo prolongado: adjacente + colado, em verde, fora de box ──
            if adjacent && inter_gap <= params.gap_close_secs && !in_yellow && !in_pit {
                acc.duel_laps.insert(snap.lap);
                if inter_gap < acc.closest_gap {
                    acc.closest_gap = inter_gap;
                }
                acc.note_fight_position(probe.position.min(other.position));
            }

            // ── Pêndulo / ultrapassagem: troca de ordem entre o par ──
            if acc.prev_sign != 0 && sign != 0 && sign != acc.prev_sign {
                // Uma inversão de ordem. Conta só fora de amarela/box.
                if !in_yellow && !in_pit {
                    acc.swaps += 1;
                    // sign agora <0 → probe passou à frente (overtake a favor).
                    if sign < 0 {
                        acc.overtakes_for += 1;
                    } else {
                        acc.overtakes_against += 1;
                    }
                    let frac = if total_laps > 0 {
                        snap.lap as f64 / total_laps as f64
                    } else {
                        0.0
                    };
                    if frac >= params.late_race_frac {
                        acc.late_overtakes += 1;
                    }
                    acc.note_fight_position(probe.position.min(other.position));
                }
            }
            if sign != 0 {
                acc.prev_sign = sign;
            }
        }
    }

    // Contato atribuído (só probe-jogador).
    if let Some(seed) = contact {
        accs.entry(seed.opponent_car_idx)
            .or_insert_with(Acc::new)
            .contact = Some(seed.tier);
    }

    // ── Monta os ledgers com deltas projetados ──
    let mut opponents: Vec<OpponentLedger> = accs
        .into_iter()
        .filter_map(|(idx, acc)| build_ledger(idx, acc, &car_number, params))
        .collect();

    opponents.sort_by(|a, b| {
        b.projected_perceived
            .partial_cmp(&a.projected_perceived)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.car_idx.cmp(&b.car_idx))
    });

    RivalryPerception {
        probe_car_idx,
        is_player_probe,
        total_laps,
        opponents,
        params: params.clone(),
    }
}

fn relevance_for(position: i32, params: &PerceptionParams) -> f64 {
    if position >= 1 && position <= params.podium_positions {
        params.relevance_podium
    } else if position >= 1 && position <= params.points_positions {
        params.relevance_points
    } else {
        params.relevance_rest
    }
}

/// Converte o acumulador num ledger. Retorna `None` se nada relevante foi percebido.
fn build_ledger(
    car_idx: i32,
    acc: Acc,
    car_number: &HashMap<i32, i32>,
    params: &PerceptionParams,
) -> Option<OpponentLedger> {
    let duel_laps = acc.duel_laps.len() as i32;
    let rel = relevance_for(acc.best_fight_position, params);
    let mut hits: Vec<SignalHit> = Vec::new();

    // Contato (não escala por relevância — batida é batida).
    if let Some(tier) = acc.contact {
        let (h, r) = tier.deltas();
        hits.push(SignalHit {
            kind: "contato".to_string(),
            detail: format!("contato {}", tier.label()),
            historical_delta: h,
            recent_delta: r,
        });
    }

    // Duelo prolongado.
    if duel_laps >= params.min_duel_laps {
        let span = (params.duel_saturation_laps - params.min_duel_laps).max(1) as f64;
        let t = ((duel_laps - params.min_duel_laps) as f64 / span).clamp(0.0, 1.0);
        let gap_txt = if acc.closest_gap.is_finite() {
            format!("{:.2}s", acc.closest_gap)
        } else {
            "—".to_string()
        };
        hits.push(SignalHit {
            kind: "duelo".to_string(),
            detail: format!("{duel_laps} voltas coladas (menor gap {gap_txt})"),
            historical_delta: lerp(2.0, 5.0, t) * rel,
            recent_delta: lerp(8.0, 14.0, t) * rel,
        });
    }

    // Pêndulo de posições.
    if acc.swaps >= params.min_swaps {
        let n = acc.swaps.min(params.swap_saturation) as f64;
        hits.push(SignalHit {
            kind: "pendulo".to_string(),
            detail: format!("{} trocas de posição", acc.swaps),
            historical_delta: 1.0 * n * rel,
            recent_delta: 3.0 * n * rel,
        });
    }

    // Ultrapassagem decisiva: tarde E por posição que importa.
    if acc.late_overtakes > 0
        && acc.best_fight_position >= 1
        && acc.best_fight_position <= params.points_positions
    {
        hits.push(SignalHit {
            kind: "ultrapassagem".to_string(),
            detail: format!(
                "{} ultrapassagem(ns) decisiva(s) por P{}",
                acc.late_overtakes, acc.best_fight_position
            ),
            historical_delta: 3.0 * rel,
            recent_delta: 10.0 * rel,
        });
    }

    if hits.is_empty() {
        return None;
    }

    let mut h_sum: f64 = hits.iter().map(|hit| hit.historical_delta).sum();
    let mut r_sum: f64 = hits.iter().map(|hit| hit.recent_delta).sum();
    let capped = h_sum > params.cap_historical || r_sum > params.cap_recent;
    h_sum = h_sum.min(params.cap_historical);
    r_sum = r_sum.min(params.cap_recent);

    Some(OpponentLedger {
        car_idx,
        car_number: car_number.get(&car_idx).copied().unwrap_or(0),
        duel_laps,
        closest_gap_secs: acc.closest_gap.is_finite().then_some(acc.closest_gap),
        swaps: acc.swaps,
        overtakes_for: acc.overtakes_for,
        overtakes_against: acc.overtakes_against,
        late_overtakes: acc.late_overtakes,
        best_fight_position: acc.best_fight_position,
        relevance_mult: rel,
        hits,
        historical_delta: h_sum,
        recent_delta: r_sum,
        projected_perceived: (h_sum * 0.4 + r_sum * 0.6).clamp(0.0, 100.0),
        capped,
    })
}

// ── Testes ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::iracing_sdk::race_monitor::{CarGapPoint, CarMeta, LapSnapshot, RaceHistory};

    /// Constrói um snapshot: `cars` = (idx, position, lap_dist_pct). `est_time` é
    /// derivado (pct·100 → volta de ~100s) e `gap` (ao líder) fica 0 de propósito —
    /// a proximidade agora vem do `lap_dist_pct`, não do F2Time.
    /// Colado ≈ pct 0.500/0.502 (~0.2s); longe ≈ 0.30/0.50 (~20s).
    fn snap(lap: i32, cars: &[(i32, i32, f64)]) -> LapSnapshot {
        LapSnapshot {
            lap,
            progress: 0.0,
            cars: cars
                .iter()
                .map(|&(idx, position, pct)| CarGapPoint {
                    idx,
                    position,
                    gap: 0.0,
                    lap_dist_pct: pct as f32,
                    est_time: (pct * 100.0) as f32,
                })
                .collect(),
        }
    }

    fn meta(idx: i32, car_number: i32) -> CarMeta {
        CarMeta {
            idx,
            is_ai: true,
            is_pace: false,
            class_id: 0,
            class_position: 0,
            car_number,
            grid_class_position: 0,
        }
    }

    fn history(
        laps: Vec<LapSnapshot>,
        cars_meta: Vec<CarMeta>,
        player_car_idx: i32,
    ) -> RaceHistory {
        RaceHistory {
            laps,
            player_laps: Vec::new(),
            player_track: Vec::new(),
            yellow_laps: Vec::new(),
            player_car_idx,
            attempt_number: 1,
            finished: true,
            outcome: "Finalizada".to_string(),
            car_laps: Vec::new(),
            cars_meta,
            track_id: 1,
            subsession_id: 1,
            qualy_laps: Vec::new(),
            pit_stops: Vec::new(),
            weather: Default::default(),
            player_sectors: Vec::new(),
        }
    }

    fn find<'a>(p: &'a RivalryPerception, idx: i32) -> Option<&'a OpponentLedger> {
        p.opponents.iter().find(|o| o.car_idx == idx)
    }

    #[test]
    fn duelo_prolongado_gera_sinal_duelo() {
        // Probe (idx 0, P4) colado ao idx 1 (P3) por 5 voltas (~0.2s); idx 2 (P8) longe.
        let laps: Vec<_> = (1..=5)
            .map(|l| snap(l, &[(0, 4, 0.500), (1, 3, 0.502), (2, 8, 0.700)]))
            .collect();
        let h = history(laps, vec![meta(0, 10), meta(1, 22), meta(2, 7)], 0);
        let p = perceive_rivalries(&h, 0, None, &PerceptionParams::default());

        let riv = find(&p, 1).expect("deveria perceber rivalidade com idx 1");
        assert_eq!(riv.duel_laps, 5);
        assert!(riv.hits.iter().any(|hit| hit.kind == "duelo"));
        assert_eq!(riv.car_number, 22);
        // Pódio (P3) → relevância alta.
        assert!((riv.relevance_mult - 1.6).abs() < 1e-9);
        // idx 2 estava longe (P8, gap 30) → nenhum sinal.
        assert!(find(&p, 2).is_none());
    }

    #[test]
    fn retardatario_nao_adjacente_nao_conta() {
        // idx 1 está COLADO na pista (pct quase igual), mas 3 posições à frente
        // (P1 vs P4): não é briga, é volta de diferença. Não pode gerar duelo.
        let laps: Vec<_> = (1..=6)
            .map(|l| snap(l, &[(0, 4, 0.500), (1, 1, 0.501)]))
            .collect();
        let h = history(laps, vec![meta(0, 10), meta(1, 22)], 0);
        let p = perceive_rivalries(&h, 0, None, &PerceptionParams::default());
        assert!(find(&p, 1).is_none(), "não-adjacente não vira duelo");
    }

    #[test]
    fn pendulo_conta_trocas_de_posicao() {
        // Probe (0) e idx 1 trocam de posição a cada volta, sempre adjacentes, mas
        // LONGE na pista (pct 0.30 vs 0.50 → ~20s) → só pêndulo, sem duelo.
        // v1: 0 à frente; v2: 1 à frente; v3: 0; v4: 1 → 3 trocas.
        let laps = vec![
            snap(1, &[(0, 3, 0.30), (1, 4, 0.50)]),
            snap(2, &[(0, 4, 0.30), (1, 3, 0.50)]),
            snap(3, &[(0, 3, 0.30), (1, 4, 0.50)]),
            snap(4, &[(0, 4, 0.30), (1, 3, 0.50)]),
        ];
        let h = history(laps, vec![meta(0, 10), meta(1, 22)], 0);
        let p = perceive_rivalries(&h, 0, None, &PerceptionParams::default());
        let riv = find(&p, 1).expect("deveria perceber pêndulo");
        assert_eq!(riv.swaps, 3);
        assert!(riv.hits.iter().any(|hit| hit.kind == "pendulo"));
    }

    #[test]
    fn ultrapassagem_tardia_por_posicao_relevante_e_decisiva() {
        // 10 voltas; na volta 9 (frac 0.9 ≥ 0.75) o probe passa o idx 1 por P2/P3.
        // Longe na pista (pct 0.30 vs 0.50) → isola o sinal de ultrapassagem.
        let mut laps: Vec<LapSnapshot> = (1..=8)
            .map(|l| snap(l, &[(0, 3, 0.30), (1, 2, 0.50)]))
            .collect();
        laps.push(snap(9, &[(0, 2, 0.30), (1, 3, 0.50)])); // troca tardia
        laps.push(snap(10, &[(0, 2, 0.30), (1, 3, 0.50)]));
        let h = history(laps, vec![meta(0, 10), meta(1, 22)], 0);
        let p = perceive_rivalries(&h, 0, None, &PerceptionParams::default());
        let riv = find(&p, 1).expect("deveria perceber");
        assert_eq!(riv.late_overtakes, 1);
        assert_eq!(riv.overtakes_for, 1);
        assert!(riv.hits.iter().any(|hit| hit.kind == "ultrapassagem"));
    }

    #[test]
    fn amarela_nao_conta_troca() {
        let laps = vec![
            snap(1, &[(0, 3, 0.500), (1, 4, 0.502)]), // colados
            snap(2, &[(0, 4, 0.500), (1, 3, 0.502)]), // troca na volta 2
        ];
        let mut h = history(laps, vec![meta(0, 10), meta(1, 22)], 0);
        h.yellow_laps = vec![2]; // volta 2 sob amarela
        let p = perceive_rivalries(&h, 0, None, &PerceptionParams::default());
        // A única troca foi sob amarela → sem pêndulo; e a volta 2 não conta duelo.
        // Volta 1 é 1 volta colada só (< min_duel_laps) → nada percebido.
        assert!(find(&p, 1).is_none());
    }

    #[test]
    fn box_nao_conta_como_briga() {
        // Troca na volta 3 porque idx 1 parou no box na volta 3 (cai de posição).
        let laps = vec![
            snap(1, &[(0, 4, 0.30), (1, 3, 0.50)]),
            snap(2, &[(0, 4, 0.30), (1, 3, 0.50)]),
            snap(3, &[(0, 3, 0.30), (1, 4, 0.50)]), // idx 1 pitou, despencou
        ];
        let mut h = history(laps, vec![meta(0, 10), meta(1, 22)], 0);
        h.pit_stops = vec![crate::iracing_sdk::tire_strategy::PitStop {
            car_idx: 1,
            lap: 3,
            stationary_secs: 22.0,
            track_wet_at_stop: false,
        }];
        let p = perceive_rivalries(&h, 0, None, &PerceptionParams::default());
        // A troca da volta 3 é de box → não vira pêndulo; nada mais colou o bastante.
        let riv = find(&p, 1);
        assert!(
            riv.map(|r| r.swaps).unwrap_or(0) == 0,
            "troca de box não deve contar"
        );
    }

    #[test]
    fn contato_gera_sinal_mesmo_sem_briga_de_pista() {
        // Sem interação de pista, mas o idx 1 bateu no jogador (probe = jogador).
        let laps = vec![snap(1, &[(0, 5, 0.50), (1, 9, 0.70)])];
        let h = history(laps, vec![meta(0, 10), meta(1, 22)], 0);
        let seed = ContactSeed {
            opponent_car_idx: 1,
            tier: ContactTier::Dnf,
        };
        let p = perceive_rivalries(&h, 0, Some(seed), &PerceptionParams::default());
        assert!(p.is_player_probe);
        let riv = find(&p, 1).expect("contato deveria criar ledger");
        assert!(riv.hits.iter().any(|hit| hit.kind == "contato"));
        // DNF = (5, 14).
        assert!((riv.historical_delta - 5.0).abs() < 1e-9);
        assert!((riv.recent_delta - 14.0).abs() < 1e-9);
    }

    #[test]
    fn cap_por_corrida_freia_inflacao() {
        // Empilha contato crítico + duelo longo no pódio → estoura os caps.
        let laps: Vec<_> = (1..=12)
            .map(|l| snap(l, &[(0, 2, 0.500), (1, 1, 0.502)]))
            .collect();
        let h = history(laps, vec![meta(0, 10), meta(1, 22)], 0);
        let seed = ContactSeed {
            opponent_car_idx: 1,
            tier: ContactTier::Critical,
        };
        let params = PerceptionParams::default();
        let p = perceive_rivalries(&h, 0, Some(seed), &params);
        let riv = find(&p, 1).unwrap();
        assert!(riv.capped, "deveria ter estourado o cap");
        assert!(riv.historical_delta <= params.cap_historical + 1e-9);
        assert!(riv.recent_delta <= params.cap_recent + 1e-9);
    }

    #[test]
    fn probe_ia_funciona_sem_ser_jogador() {
        // Probe = idx 2 (uma IA), jogador é idx 0. Duelo entre idx 2 e idx 3.
        let laps: Vec<_> = (1..=5)
            .map(|l| snap(l, &[(0, 1, 0.10), (2, 5, 0.500), (3, 6, 0.502)]))
            .collect();
        let h = history(laps, vec![meta(0, 10), meta(2, 33), meta(3, 44)], 0);
        let p = perceive_rivalries(&h, 2, None, &PerceptionParams::default());
        assert!(!p.is_player_probe);
        assert!(
            find(&p, 3).is_some(),
            "percepção deve rodar para uma IA-sonda"
        );
        assert!(
            find(&p, 0).is_none(),
            "jogador (P1, longe) não brigou com o probe"
        );
    }
}
