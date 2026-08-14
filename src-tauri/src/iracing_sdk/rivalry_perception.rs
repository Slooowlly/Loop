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
///
/// **Só `Dnf` e `Major` são construídos hoje.** Os dois produtores reais
/// (`commands::iracing::corridas_salvas` e `commands::iracing::resultado`) decidem pela
/// única evidência que o monitor entrega — o jogador correu e não viu a bandeirada, ou
/// viu —, então `Critical` e `Minor` ficam sem produtor e os braços deles em [`deltas`]
/// e [`label`] estão inalcançáveis em produção.
///
/// [`Self::Critical`] e [`Self::Minor`] ficam de pé de propósito: os quatro tiers
/// espelham `process_collisions_rivalry`, e apagar dois deles mudaria a tabela de
/// severidades que o motor de rivalidade compartilha. O tier fino entra quando a fase de
/// calibração do contato der ao monitor evidência para separar batida leve de batida
/// crítica — ver o comentário no ponto de decisão em `corridas_salvas.rs`.
///
/// [`deltas`]: ContactTier::deltas
/// [`label`]: ContactTier::label
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

    /// O MESMO par visto da outra ponta. Toda a métrica de par é simétrica (voltas
    /// coladas, menor gap, trocas, posição disputada); só as ultrapassagens a favor e
    /// contra trocam de lado, e o sinal da ordem inverte. É essa simetria que permite
    /// varrer um par UMA vez quando as duas pontas são sonda.
    ///
    /// O contato não entra: é semente da SONDA (quem bateu em mim), não estado do par,
    /// e por isso é aplicado depois da varredura, por sonda.
    fn mirrored(&self) -> Acc {
        Acc {
            duel_laps: self.duel_laps.clone(),
            closest_gap: self.closest_gap,
            swaps: self.swaps,
            overtakes_for: self.overtakes_against,
            overtakes_against: self.overtakes_for,
            late_overtakes: self.late_overtakes,
            best_fight_position: self.best_fight_position,
            prev_sign: -self.prev_sign,
            contact: None,
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

// ── Varredura multi-sonda ─────────────────────────────────────────────────────

/// Uma sonda pedida à varredura. `contact` só existe para o probe-jogador.
#[derive(Clone, Copy, Debug)]
pub struct Probe {
    pub car_idx: i32,
    pub contact: Option<ContactSeed>,
}

impl Probe {
    /// Sonda sem contato atribuído — o caso de qualquer IA.
    pub fn new(car_idx: i32) -> Self {
        Probe {
            car_idx,
            contact: None,
        }
    }
}

/// O que a varredura custou. Existe para o custo APARECER no log antes de o grid de
/// endurance fazer doer: `snapshot_passes` é 1 por chamada, quantas sondas forem, e
/// `pair_evaluations` é o trabalho real, que já vem com o par contado uma vez só.
#[derive(Clone, Copy, Debug, Default, Serialize)]
pub struct ScanCost {
    /// Passadas completas sobre `history.laps`. Sempre 1 (0 se não veio sonda).
    pub snapshot_passes: usize,
    /// Sondas efetivamente varridas (sonda repetida conta uma vez).
    pub probes: usize,
    /// Pares (sonda, carro) avaliados somando todos os snapshots.
    pub pair_evaluations: usize,
    /// Pares cujo lado espelhado saiu de graça (as duas pontas eram sonda).
    pub mirrored_pairs: usize,
}

/// Saída da varredura multi-sonda.
#[derive(Clone, Debug, Serialize)]
pub struct MultiPerception {
    /// Uma percepção por sonda ÚNICA, na ordem da primeira ocorrência em `probes`.
    pub probes: Vec<RivalryPerception>,
    pub cost: ScanCost,
}

/// Contexto de um snapshot, comum a todos os pares avaliados nele.
struct SnapCtx<'a> {
    lap: i32,
    in_yellow: bool,
    /// Fração da corrida já corrida neste snapshot (para "ultrapassagem tarde").
    race_frac: f64,
    params: &'a PerceptionParams,
}

/// Acumula UM par num snapshot, do ponto de vista de `dono`. A outra ponta lê o mesmo
/// estado espelhado ([`Acc::mirrored`]) — nada aqui depende de quem é a sonda.
fn acumula_par(
    acc: &mut Acc,
    dono: &CarGapPoint,
    outro: &CarGapPoint,
    in_pit: bool,
    ctx: &SnapCtx<'_>,
) {
    let pos_diff = dono.position - outro.position; // <0: dono à frente
    let sign = pos_diff.signum();
    let adjacent = pos_diff.abs() == 1;
    let inter_gap = track_gap_secs(dono, outro);

    // ── Duelo prolongado: adjacente + colado, em verde, fora de box ──
    if adjacent && inter_gap <= ctx.params.gap_close_secs && !ctx.in_yellow && !in_pit {
        acc.duel_laps.insert(ctx.lap);
        if inter_gap < acc.closest_gap {
            acc.closest_gap = inter_gap;
        }
        acc.note_fight_position(dono.position.min(outro.position));
    }

    // ── Pêndulo / ultrapassagem: troca de ordem entre o par ──
    // Uma inversão de ordem. Conta só fora de amarela/box.
    if acc.prev_sign != 0 && sign != 0 && sign != acc.prev_sign && !ctx.in_yellow && !in_pit {
        acc.swaps += 1;
        // sign agora <0 → o dono passou à frente (overtake a favor DELE).
        if sign < 0 {
            acc.overtakes_for += 1;
        } else {
            acc.overtakes_against += 1;
        }
        if ctx.race_frac >= ctx.params.late_race_frac {
            acc.late_overtakes += 1;
        }
        acc.note_fight_position(dono.position.min(outro.position));
    }
    if sign != 0 {
        acc.prev_sign = sign;
    }
}

/// Percebe as rivalidades de VÁRIAS sondas numa varredura só. Puro.
///
/// Uma passada sobre `history.laps` atende todas as sondas, e cada par de carros é
/// avaliado UMA vez mesmo quando as duas pontas são sonda — o lado espelhado sai por
/// simetria ([`Acc::mirrored`]). No lugar de `sondas × snapshots` passadas com metade
/// do resultado jogada fora, fica uma passada com o par contado uma vez.
///
/// A percepção de cada sonda é IDÊNTICA à de [`perceive_rivalries`] chamada
/// isoladamente; o teste `multi_sonda_equivale_a_uma_chamada_por_sonda` trava isso
/// contra uma cópia literal do laço de sonda única.
///
/// Sonda repetida em `probes` é varrida uma vez (a primeira ocorrência manda, inclusive
/// o contato dela) e sai uma vez do resultado.
pub fn perceive_rivalries_multi(
    history: &RaceHistory,
    probes: &[Probe],
    params: &PerceptionParams,
) -> MultiPerception {
    let total_laps = history.laps.iter().map(|s| s.lap).max().unwrap_or(0);

    // Máscaras de ruído — iguais para todas as sondas, montadas uma vez.
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

    // Ordem canônica das sondas: define qual ponta é "dona" de um par entre duas sondas.
    let mut probe_order: Vec<i32> = Vec::with_capacity(probes.len());
    let mut probe_rank: HashMap<i32, usize> = HashMap::with_capacity(probes.len());
    let mut contatos: HashMap<i32, ContactSeed> = HashMap::new();
    for p in probes {
        if probe_rank.contains_key(&p.car_idx) {
            continue;
        }
        probe_rank.insert(p.car_idx, probe_order.len());
        probe_order.push(p.car_idx);
        if let Some(seed) = p.contact {
            contatos.insert(p.car_idx, seed);
        }
    }

    let mut cost = ScanCost {
        snapshot_passes: usize::from(!probe_order.is_empty()),
        probes: probe_order.len(),
        ..Default::default()
    };

    // (dono, outro) → estado do par, sempre do ponto de vista do dono.
    let mut pares: HashMap<(i32, i32), Acc> = HashMap::new();

    for snap in &history.laps {
        let ctx = SnapCtx {
            lap: snap.lap,
            in_yellow: yellow.contains(&snap.lap),
            race_frac: if total_laps > 0 {
                snap.lap as f64 / total_laps as f64
            } else {
                0.0
            },
            params,
        };

        // Varre os carros do snapshot e trabalha a partir dos que são sonda. Procurar cada
        // sonda com um `find` custaria outro `sondas × carros` por snapshot, e num grid de
        // enduro essa busca sozinha passaria o trabalho de par que ela serve.
        for dono in &snap.cars {
            let Some(&rank) = probe_rank.get(&dono.idx) else {
                continue;
            };
            let probe_idx = dono.idx;
            let dono_no_box = pit_near(probe_idx, snap.lap);

            for outro in &snap.cars {
                if outro.idx == probe_idx {
                    continue;
                }
                // Se o outro também é sonda e veio antes na ordem, o par já foi varrido
                // por ele — é exatamente a metade que o dedupe do chamador jogava fora.
                if matches!(probe_rank.get(&outro.idx), Some(&r) if r < rank) {
                    continue;
                }
                cost.pair_evaluations += 1;
                let acc = pares.entry((probe_idx, outro.idx)).or_insert_with(Acc::new);
                acumula_par(
                    acc,
                    dono,
                    outro,
                    dono_no_box || pit_near(outro.idx, snap.lap),
                    &ctx,
                );
            }
        }
    }

    // ── Espalha cada par para as pontas que são sonda ──
    let mut por_sonda: HashMap<i32, HashMap<i32, Acc>> = probe_order
        .iter()
        .map(|&idx| (idx, HashMap::new()))
        .collect();
    for ((dono, outro), acc) in pares {
        if probe_rank.contains_key(&outro) {
            cost.mirrored_pairs += 1;
            if let Some(m) = por_sonda.get_mut(&outro) {
                m.insert(dono, acc.mirrored());
            }
        }
        if let Some(m) = por_sonda.get_mut(&dono) {
            m.insert(outro, acc);
        }
    }

    // Contato atribuído (só o probe-jogador tem semente).
    for (probe_idx, seed) in contatos {
        if let Some(m) = por_sonda.get_mut(&probe_idx) {
            m.entry(seed.opponent_car_idx)
                .or_insert_with(Acc::new)
                .contact = Some(seed.tier);
        }
    }

    // ── Monta os ledgers com deltas projetados ──
    let mut resultados: Vec<RivalryPerception> = Vec::with_capacity(probe_order.len());
    for probe_car_idx in probe_order {
        let accs = por_sonda.remove(&probe_car_idx).unwrap_or_default();
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

        resultados.push(RivalryPerception {
            probe_car_idx,
            is_player_probe: probe_car_idx == history.player_car_idx,
            total_laps,
            opponents,
            params: params.clone(),
        });
    }

    MultiPerception {
        probes: resultados,
        cost,
    }
}

/// Percebe as rivalidades de `probe_car_idx` numa corrida. Puro.
///
/// Casca de uma sonda sobre [`perceive_rivalries_multi`] — com uma sonda só não há
/// espelhamento, cada par é varrido do jeito direto.
pub fn perceive_rivalries(
    history: &RaceHistory,
    probe_car_idx: i32,
    contact: Option<ContactSeed>,
    params: &PerceptionParams,
) -> RivalryPerception {
    let mut multi = perceive_rivalries_multi(
        history,
        &[Probe {
            car_idx: probe_car_idx,
            contact,
        }],
        params,
    );
    multi
        .probes
        .pop()
        .expect("uma sonda pedida, uma percepção devolvida")
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
            qualy_best_valid: Vec::new(),
            pit_stops: Vec::new(),
            weather: Default::default(),
            player_sectors: Vec::new(),
        }
    }

    fn find<'a>(p: &'a RivalryPerception, idx: i32) -> Option<&'a OpponentLedger> {
        p.opponents.iter().find(|o| o.car_idx == idx)
    }

    // ── Padrão-ouro da equivalência ───────────────────────────────────────────

    /// Cópia LITERAL do laço de sonda única que existia antes da varredura
    /// multi-sonda. Não é para ser mantida bonita nem refatorada junto: é a régua
    /// contra a qual a percepção nova se mede. Se a percepção de alguma sonda mudar,
    /// é aqui que aparece.
    fn perceive_referencia(
        history: &RaceHistory,
        probe_car_idx: i32,
        contact: Option<ContactSeed>,
        params: &PerceptionParams,
    ) -> RivalryPerception {
        let is_player_probe = probe_car_idx == history.player_car_idx;
        let total_laps = history.laps.iter().map(|s| s.lap).max().unwrap_or(0);

        let yellow: HashSet<i32> = history.yellow_laps.iter().copied().collect();
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

        let car_number: HashMap<i32, i32> = history
            .cars_meta
            .iter()
            .map(|c| (c.idx, c.car_number))
            .collect();

        let mut accs: HashMap<i32, Acc> = HashMap::new();

        for snap in &history.laps {
            let in_yellow = yellow.contains(&snap.lap);
            let Some(probe) = snap.cars.iter().find(|c| c.idx == probe_car_idx) else {
                continue;
            };

            for other in &snap.cars {
                if other.idx == probe_car_idx {
                    continue;
                }
                let acc = accs.entry(other.idx).or_insert_with(Acc::new);

                let pos_diff = probe.position - other.position;
                let sign = pos_diff.signum();
                let adjacent = pos_diff.abs() == 1;
                let inter_gap = track_gap_secs(probe, other);
                let in_pit = pit_near(probe_car_idx, snap.lap) || pit_near(other.idx, snap.lap);

                if adjacent && inter_gap <= params.gap_close_secs && !in_yellow && !in_pit {
                    acc.duel_laps.insert(snap.lap);
                    if inter_gap < acc.closest_gap {
                        acc.closest_gap = inter_gap;
                    }
                    acc.note_fight_position(probe.position.min(other.position));
                }

                if acc.prev_sign != 0 && sign != 0 && sign != acc.prev_sign {
                    if !in_yellow && !in_pit {
                        acc.swaps += 1;
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

        if let Some(seed) = contact {
            accs.entry(seed.opponent_car_idx)
                .or_insert_with(Acc::new)
                .contact = Some(seed.tier);
        }

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

    /// Compara duas percepções campo a campo, inclusive a ordem dos oponentes e a
    /// prosa dos sinais (o explicador de debug faz parte do contrato).
    fn assert_percepcao_igual(novo: &RivalryPerception, referencia: &RivalryPerception, ctx: &str) {
        assert_eq!(novo.probe_car_idx, referencia.probe_car_idx, "{ctx}: probe");
        assert_eq!(
            novo.is_player_probe, referencia.is_player_probe,
            "{ctx}: is_player_probe"
        );
        assert_eq!(novo.total_laps, referencia.total_laps, "{ctx}: total_laps");
        assert_eq!(
            novo.opponents.len(),
            referencia.opponents.len(),
            "{ctx}: quantidade de oponentes"
        );
        for (a, b) in novo.opponents.iter().zip(referencia.opponents.iter()) {
            let onde = format!("{ctx}, oponente {}", b.car_idx);
            assert_eq!(a.car_idx, b.car_idx, "{onde}: car_idx");
            assert_eq!(a.car_number, b.car_number, "{onde}: car_number");
            assert_eq!(a.duel_laps, b.duel_laps, "{onde}: duel_laps");
            assert_eq!(
                a.closest_gap_secs, b.closest_gap_secs,
                "{onde}: closest_gap_secs"
            );
            assert_eq!(a.swaps, b.swaps, "{onde}: swaps");
            assert_eq!(a.overtakes_for, b.overtakes_for, "{onde}: overtakes_for");
            assert_eq!(
                a.overtakes_against, b.overtakes_against,
                "{onde}: overtakes_against"
            );
            assert_eq!(a.late_overtakes, b.late_overtakes, "{onde}: late_overtakes");
            assert_eq!(
                a.best_fight_position, b.best_fight_position,
                "{onde}: best_fight_position"
            );
            assert_eq!(a.relevance_mult, b.relevance_mult, "{onde}: relevance_mult");
            assert_eq!(a.historical_delta, b.historical_delta, "{onde}: historical");
            assert_eq!(a.recent_delta, b.recent_delta, "{onde}: recent");
            assert_eq!(
                a.projected_perceived, b.projected_perceived,
                "{onde}: projected"
            );
            assert_eq!(a.capped, b.capped, "{onde}: capped");
            assert_eq!(a.hits.len(), b.hits.len(), "{onde}: quantidade de sinais");
            for (ha, hb) in a.hits.iter().zip(b.hits.iter()) {
                assert_eq!(ha.kind, hb.kind, "{onde}: kind do sinal");
                assert_eq!(ha.detail, hb.detail, "{onde}: detail do sinal");
                assert_eq!(
                    ha.historical_delta, hb.historical_delta,
                    "{onde}: historical do sinal"
                );
                assert_eq!(ha.recent_delta, hb.recent_delta, "{onde}: recent do sinal");
            }
        }
    }

    /// Gerador determinístico (sem `rand`): o cenário precisa ser o mesmo em toda
    /// execução para a equivalência valer alguma coisa.
    struct Lcg(u64);

    impl Lcg {
        fn proximo(&mut self) -> u32 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (self.0 >> 33) as u32
        }
    }

    /// Histórico sintético denso: `n_cars` carros embaralhando de posição a cada volta,
    /// com o progresso na pista espalhado o bastante para gerar duelos colados,
    /// pêndulos e ultrapassagens tardias em vários pares ao mesmo tempo.
    fn historico_denso(n_cars: i32, n_laps: i32) -> RaceHistory {
        let mut rng = Lcg(0x5eed_1ace);
        let mut ordem: Vec<i32> = (0..n_cars).collect();
        let mut laps: Vec<LapSnapshot> = Vec::new();

        for l in 1..=n_laps {
            for _ in 0..3 {
                let i = (rng.proximo() as usize) % (n_cars as usize - 1);
                ordem.swap(i, i + 1);
            }
            let cars: Vec<(i32, i32, f64)> = ordem
                .iter()
                .enumerate()
                .map(|(pos, &idx)| {
                    // Espaçamento base minúsculo (vira duelo colado) com um empurrão
                    // ocasional que joga o carro para longe na pista.
                    let solto = (rng.proximo() % 4 == 0) as u32 as f64;
                    let pct = 0.20 + pos as f64 * 0.0015 + solto * 0.12;
                    (idx, pos as i32 + 1, pct)
                })
                .collect();
            laps.push(snap(l, &cars));
        }

        let metas: Vec<CarMeta> = (0..n_cars).map(|i| meta(i, 100 + i)).collect();
        history(laps, metas, 0)
    }

    fn parada(car_idx: i32, lap: i32) -> crate::iracing_sdk::tire_strategy::PitStop {
        crate::iracing_sdk::tire_strategy::PitStop {
            car_idx,
            lap,
            stationary_secs: 24.0,
            track_wet_at_stop: false,
        }
    }

    #[test]
    fn multi_sonda_equivale_a_uma_chamada_por_sonda() {
        let mut h = historico_denso(12, 30);
        h.yellow_laps = vec![7, 8, 19];
        h.pit_stops = vec![parada(3, 12), parada(5, 13), parada(0, 14)];
        let params = PerceptionParams::default();
        let contato = ContactSeed {
            opponent_car_idx: 4,
            tier: ContactTier::Major,
        };

        let probes: Vec<Probe> = (0..12)
            .map(|idx| Probe {
                car_idx: idx,
                contact: (idx == 0).then_some(contato),
            })
            .collect();
        let multi = perceive_rivalries_multi(&h, &probes, &params);
        assert_eq!(multi.probes.len(), probes.len());

        for (i, probe) in probes.iter().enumerate() {
            let idx = probe.car_idx;
            let referencia = perceive_referencia(&h, idx, probe.contact, &params);
            assert_percepcao_igual(&multi.probes[i], &referencia, &format!("sonda {idx}"));
            // E a casca de uma sonda também continua batendo com o laço antigo.
            let uma = perceive_rivalries(&h, idx, probe.contact, &params);
            assert_percepcao_igual(&uma, &referencia, &format!("sonda única {idx}"));
        }

        // Sanidade do cenário: equivalência de percepção vazia não prova nada.
        let sinais: usize = multi.probes.iter().map(|p| p.opponents.len()).sum();
        assert!(
            sinais >= 12,
            "cenário fraco demais para provar equivalência ({sinais} ledgers)"
        );
        assert!(
            multi
                .probes
                .iter()
                .flat_map(|p| p.opponents.iter())
                .flat_map(|o| o.hits.iter())
                .any(|hit| hit.kind == "duelo"),
            "o cenário precisa ter pelo menos um duelo"
        );
    }

    #[test]
    fn sonda_repetida_e_varrida_uma_vez_so() {
        let h = historico_denso(6, 12);
        let params = PerceptionParams::default();
        let repetida = perceive_rivalries_multi(
            &h,
            &[Probe::new(1), Probe::new(1), Probe::new(2), Probe::new(1)],
            &params,
        );
        let limpa = perceive_rivalries_multi(&h, &[Probe::new(1), Probe::new(2)], &params);
        assert_eq!(repetida.probes.len(), 2, "sonda repetida sai uma vez");
        assert_eq!(repetida.cost.probes, 2);
        assert_eq!(
            repetida.cost.pair_evaluations, limpa.cost.pair_evaluations,
            "repetir a sonda não pode custar varredura a mais"
        );
        for (a, b) in repetida.probes.iter().zip(limpa.probes.iter()) {
            assert_percepcao_igual(a, b, "sonda repetida");
        }
    }

    #[test]
    fn lado_espelhado_troca_as_ultrapassagens_de_lado() {
        // 10 voltas; na volta 9 o probe 0 passa o 1. Visto do 1, a mesma troca é uma
        // ultrapassagem CONTRA.
        let mut laps: Vec<LapSnapshot> = (1..=8)
            .map(|l| snap(l, &[(0, 3, 0.30), (1, 2, 0.50)]))
            .collect();
        laps.push(snap(9, &[(0, 2, 0.30), (1, 3, 0.50)]));
        laps.push(snap(10, &[(0, 2, 0.30), (1, 3, 0.50)]));
        let h = history(laps, vec![meta(0, 10), meta(1, 22)], 0);

        let m = perceive_rivalries_multi(
            &h,
            &[Probe::new(0), Probe::new(1)],
            &PerceptionParams::default(),
        );
        assert_eq!(m.cost.mirrored_pairs, 1, "o par tem as duas pontas sonda");

        let visto_do_0 = find(&m.probes[0], 1).expect("sonda 0 vê o 1");
        let visto_do_1 = find(&m.probes[1], 0).expect("sonda 1 vê o 0");
        assert_eq!(visto_do_0.overtakes_for, 1);
        assert_eq!(visto_do_0.overtakes_against, 0);
        assert_eq!(visto_do_1.overtakes_for, 0);
        assert_eq!(visto_do_1.overtakes_against, 1);
        // O resto do par é simétrico e tem que bater dos dois lados.
        assert_eq!(visto_do_0.swaps, visto_do_1.swaps);
        assert_eq!(visto_do_0.late_overtakes, visto_do_1.late_overtakes);
        assert_eq!(
            visto_do_0.best_fight_position,
            visto_do_1.best_fight_position
        );
        assert_eq!(visto_do_0.historical_delta, visto_do_1.historical_delta);
    }

    #[test]
    fn passadas_nao_crescem_com_sondas_vezes_snapshots() {
        const CARROS: usize = 16;
        let h = historico_denso(CARROS as i32, 40);
        let params = PerceptionParams::default();
        let snapshots = h.laps.len();

        for n in [1usize, 2, 4, 8, CARROS] {
            let probes: Vec<Probe> = (0..n as i32).map(Probe::new).collect();
            let m = perceive_rivalries_multi(&h, &probes, &params);

            // A prova do item: UMA passada sobre os snapshots, quantas sondas forem.
            assert_eq!(m.cost.snapshot_passes, 1, "{n} sondas");
            assert_eq!(m.cost.probes, n, "{n} sondas");

            // O laço antigo fazia uma passada por sonda, cada uma avaliando todos os
            // outros carros: sondas × snapshots × (carros-1).
            let antigo = n * snapshots * (CARROS - 1);
            // O novo desconta os pares em que as duas pontas são sonda (varridos uma vez).
            let esperado = snapshots * (n * (CARROS - 1) - n * (n - 1) / 2);
            assert_eq!(m.cost.pair_evaluations, esperado, "{n} sondas");
            assert!(
                m.cost.pair_evaluations <= antigo,
                "{n} sondas: {} avaliações contra {antigo} do laço antigo",
                m.cost.pair_evaluations
            );
        }

        // Sondando o grid inteiro — o caso do import de corrida — o corte é de metade
        // exata: todo par é avaliado uma vez e espelhado para a outra ponta.
        let todas: Vec<Probe> = (0..CARROS as i32).map(Probe::new).collect();
        let m = perceive_rivalries_multi(&h, &todas, &params);
        let pares_do_grid = CARROS * (CARROS - 1) / 2;
        assert_eq!(m.cost.pair_evaluations, snapshots * pares_do_grid);
        assert_eq!(m.cost.mirrored_pairs, pares_do_grid);
        assert_eq!(
            m.cost.pair_evaluations * 2,
            CARROS * snapshots * (CARROS - 1),
            "metade exata do que o laço antigo avaliava"
        );
    }

    #[test]
    fn sem_sonda_nao_varre_nada() {
        let h = historico_denso(6, 10);
        let m = perceive_rivalries_multi(&h, &[], &PerceptionParams::default());
        assert!(m.probes.is_empty());
        assert_eq!(m.cost.snapshot_passes, 0);
        assert_eq!(m.cost.pair_evaluations, 0);
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
