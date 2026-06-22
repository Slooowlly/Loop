//! Camada ADAPTATIVA de dificuldade da IA — **Fase B: lógica pura, testável**.
//!
//! Recebe um resultado de corrida (tempos de volta por piloto, classe, posição,
//! amarelas, quali opcional) e devolve quanto mexer nos deltas de skill do save.
//! Sem I/O e sem dependência do SDK — a Fase A é quem preenche o [`RaceResult`].
//!
//! ## Como funciona (design travado com o user)
//! O skill final da IA = `base_tier + offset_pista + delta_global + delta_track`.
//! Esta camada só calcula como `delta_global`/`delta_track` evoluem. Filosofia:
//! a IA tem que ser SEMPRE competitiva — sobe fácil, desce pouco e com má vontade.
//!
//! - **Sinal = RITMO** nas voltas verdes vs a **mediana dos 2 melhores da classe**
//!   (robusto a multiclasse, tamanho de grid, amarela e batida de fim de corrida).
//! - **Ritmo médio = porteiro pra SUBIR** (vantagem sustentada; volta voadora não
//!   sobe, pois a mediana é robusta a uma volta solta).
//! - **Melhor volta = ESCUDO contra DESCER** (nunca sobe sozinha): se o ritmo médio
//!   foi ruim mas a melhor volta foi competitiva, foi TRÂNSITO → não desce.
//! - **Equilíbrio**: vencer apertado/brigar no topo não mexe. Só DOMINAR sobe.
//! - **Erro ≠ ritmo**: descarta voltas muito mais lentas que a mediana do próprio
//!   piloto (rodada/saída), em vez de confiar na invalidação do iRacing.

use serde::{Deserialize, Serialize};

// ─── Parâmetros (ajustáveis) ────────────────────────────────────────────────

/// Volta mais lenta que `mediana + isto` (s) é erro (rodada/saída) → descartada.
const OUTLIER_SECONDS: f64 = 4.0;
/// Mínimo de voltas limpas pra a amostra de ritmo valer.
const MIN_CLEAN_LAPS: usize = 2;

/// Limiares de gap de ritmo (jogador − referência), em s/volta (negativo = + rápido).
const G_DOMINATE: f64 = -0.7; // ≤ isto = dominou
const G_CLEAR: f64 = -0.3; // ≤ isto = venceu com folga
const G_BEHIND: f64 = 0.3; // < isto (e ≥ G_CLEAR) = equilíbrio; ≥ isto = atrás
const G_FAR: f64 = 1.0; // ≥ isto = muito atrás
/// Escudo da melhor volta: se o best_gap fica abaixo disto, foi trânsito → não desce.
const SHIELD: f64 = 0.3;

/// Piso ASSIMÉTRICO (desce pouco, sobe muito = sempre competitivo).
const GLOBAL_MIN: i64 = -5;
const GLOBAL_MAX: i64 = 15;
const TRACK_MIN: i64 = -3;
const TRACK_MAX: i64 = 8;
/// Acima deste delta global, o passo de SUBIDA é amortecido (no máx +1/corrida),
/// pra estabilizar e não punir quem vai bem.
const SOFT_CAP_GLOBAL: i64 = 10;

// ─── Entrada (o que a Fase A vai preencher) ─────────────────────────────────

/// Uma volta com seu número (pra filtrar volta 1 e amarelas).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Lap {
    pub lap: i32,
    pub time: f64,
}

/// Dados de um piloto numa sessão (corrida ou quali).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DriverData {
    pub car_idx: i32,
    pub is_player: bool,
    pub is_ai: bool,
    pub car_class_id: i64,
    /// Posição final NA CLASSE (1-based). Em quali pode ser 0 (não usado lá).
    pub finish_pos_in_class: i32,
    pub dnf: bool,
    pub laps: Vec<Lap>,
}

/// Resultado de uma corrida (+ quali opcional), pronto para a adaptação.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RaceResult {
    pub track_id: i64,
    /// Voltas (do líder) em que a amarela esteve ativa — excluídas do ritmo.
    pub yellow_laps: Vec<i32>,
    pub race: Vec<DriverData>,
    /// Quali opcional: mesmas DriverData, mas com as voltas de qualificação.
    pub qualy: Option<Vec<DriverData>>,
}

/// Os deltas atuais do save (antes desta corrida).
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Deltas {
    pub global: i64,
    pub track: i64,
}

// ─── Saída ──────────────────────────────────────────────────────────────────

/// O que mudar nos deltas depois de uma corrida.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AdaptiveUpdate {
    /// Deltas resultantes (já com piso/teto aplicados).
    pub new: Deltas,
    /// Quanto de fato mudou (depois do clamp).
    pub d_global: i64,
    pub d_track: i64,
    /// Se a corrida foi válida para adaptar (false = DNF/dados insuficientes).
    pub applied: bool,
    /// Explicação legível (UI/log).
    pub verdict: String,
}

impl AdaptiveUpdate {
    fn unchanged(current: &Deltas, verdict: impl Into<String>) -> Self {
        AdaptiveUpdate {
            new: *current,
            d_global: 0,
            d_track: 0,
            applied: false,
            verdict: verdict.into(),
        }
    }
}

// ─── Lógica ─────────────────────────────────────────────────────────────────

fn median(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut v: Vec<f64> = values.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = v.len();
    Some(if n % 2 == 1 {
        v[n / 2]
    } else {
        (v[n / 2 - 1] + v[n / 2]) / 2.0
    })
}

/// Ritmo limpo de um piloto: descarta volta 1 (largada), amarelas, tempos ≤0 e
/// erros (voltas > mediana + OUTLIER_SECONDS). Devolve `(ritmo_típico, melhor)`.
fn clean_pace(laps: &[Lap], yellow: &[i32]) -> Option<(f64, f64)> {
    if laps.is_empty() {
        return None;
    }
    let first_lap = laps.iter().map(|l| l.lap).min().unwrap_or(i32::MIN);
    let mut pool: Vec<f64> = laps
        .iter()
        .filter(|l| l.lap != first_lap && !yellow.contains(&l.lap) && l.time > 0.0)
        .map(|l| l.time)
        .collect();
    if pool.len() < MIN_CLEAN_LAPS {
        return None;
    }
    // Tira erros (rodada/saída) sem confiar na invalidação do iRacing.
    let med = median(&pool)?;
    pool.retain(|&t| t <= med + OUTLIER_SECONDS);
    if pool.len() < MIN_CLEAN_LAPS {
        return None;
    }
    let typical = median(&pool)?;
    let best = pool.iter().cloned().fold(f64::INFINITY, f64::min);
    Some((typical, best))
}

/// Só a melhor volta limpa de um piloto (usada pra quali, como reforço do escudo).
fn clean_best(laps: &[Lap], yellow: &[i32]) -> Option<f64> {
    clean_pace(laps, yellow).map(|(_, best)| best)
}

/// Referência da classe = mediana (= média, p/ 2) do ritmo e da melhor volta dos
/// 2 melhores colocados da classe do jogador (IA, com amostra válida).
fn class_reference(result: &RaceResult, class_id: i64) -> Option<(f64, f64)> {
    let mut rivals: Vec<(i32, f64, f64)> = result
        .race
        .iter()
        .filter(|d| d.is_ai && d.car_class_id == class_id && !d.dnf)
        .filter_map(|d| {
            clean_pace(&d.laps, &result.yellow_laps).map(|(t, b)| (d.finish_pos_in_class, t, b))
        })
        .collect();
    if rivals.is_empty() {
        return None;
    }
    rivals.sort_by_key(|r| r.0);
    rivals.truncate(2);
    let n = rivals.len() as f64;
    let ref_typ = rivals.iter().map(|r| r.1).sum::<f64>() / n;
    let ref_best = rivals.iter().map(|r| r.2).sum::<f64>() / n;
    Some((ref_typ, ref_best))
}

/// Melhor volta de referência na QUALI = média das 2 melhores voltas limpas de
/// quali entre os carros de IA da classe.
fn class_qualy_best(qualy: &[DriverData], class_id: i64) -> Option<f64> {
    let mut bests: Vec<f64> = qualy
        .iter()
        .filter(|d| d.is_ai && d.car_class_id == class_id)
        .filter_map(|d| clean_best(&d.laps, &[]))
        .collect();
    if bests.is_empty() {
        return None;
    }
    bests.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    bests.truncate(2);
    Some(bests.iter().sum::<f64>() / bests.len() as f64)
}

/// Núcleo: calcula como os deltas evoluem dada uma corrida e os deltas atuais.
pub fn compute_adaptive_update(result: &RaceResult, current: &Deltas) -> AdaptiveUpdate {
    // Porta 1: jogador presente e SEM DNF (corrida representativa).
    let player = match result.race.iter().find(|d| d.is_player) {
        Some(p) if !p.dnf => p,
        Some(_) => {
            return AdaptiveUpdate::unchanged(current, "Jogador não terminou (DNF) → sem ajuste")
        }
        None => {
            return AdaptiveUpdate::unchanged(current, "Jogador ausente nos dados → sem ajuste")
        }
    };

    // Ritmo limpo do jogador + referência da classe.
    let (player_typ, player_best) = match clean_pace(&player.laps, &result.yellow_laps) {
        Some(p) => p,
        None => {
            return AdaptiveUpdate::unchanged(
                current,
                "Poucas voltas limpas do jogador → sem ajuste",
            )
        }
    };
    let (ref_typ, ref_best) = match class_reference(result, player.car_class_id) {
        Some(r) => r,
        None => {
            return AdaptiveUpdate::unchanged(
                current,
                "Sem referência válida na classe → sem ajuste",
            )
        }
    };

    // Melhor volta "potencial" = melhor da corrida OU da quali (o que for mais
    // rápido) — quali limpa reforça o escudo quando a corrida teve trânsito.
    let player_pot = match result.qualy.as_deref().and_then(|q| {
        q.iter()
            .find(|d| d.is_player)
            .and_then(|d| clean_best(&d.laps, &[]))
    }) {
        Some(q) => player_best.min(q),
        None => player_best,
    };
    let ref_pot = match result
        .qualy
        .as_deref()
        .and_then(|q| class_qualy_best(q, player.car_class_id))
    {
        Some(q) => ref_best.min(q),
        None => ref_best,
    };

    let avg_gap = player_typ - ref_typ; // negativo = jogador mais rápido
    let best_gap = player_pot - ref_pot;

    // Decisão. Ritmo médio é o porteiro pra subir; melhor volta é o escudo p/ descer.
    let (raw_global, raw_track, why): (i64, i64, String) = if avg_gap <= G_DOMINATE {
        (
            2,
            2,
            format!("Dominou (ritmo {avg_gap:+.2}s/volta) → sobe forte"),
        )
    } else if avg_gap <= G_CLEAR {
        (
            1,
            1,
            format!("Venceu com folga (ritmo {avg_gap:+.2}s/volta) → sobe"),
        )
    } else if avg_gap < G_BEHIND {
        (
            0,
            0,
            format!("Equilíbrio (ritmo {avg_gap:+.2}s/volta) → mantém"),
        )
    } else if best_gap < SHIELD {
        (
            0,
            0,
            format!("Trânsito: ritmo {avg_gap:+.2}s mas melhor volta competitiva ({best_gap:+.2}s) → mantém"),
        )
    } else if avg_gap < G_FAR {
        (
            -1,
            0,
            format!("Abaixo do ritmo ({avg_gap:+.2}s/volta) → desce"),
        )
    } else {
        (
            -1,
            -1,
            format!("Muito atrás ({avg_gap:+.2}s/volta) → desce"),
        )
    };

    // Amortece a subida perto do teto (não punir quem vai bem).
    let damped_global = if raw_global > 0 && current.global >= SOFT_CAP_GLOBAL {
        raw_global.min(1)
    } else {
        raw_global
    };

    // Aplica piso/teto e mede a mudança real.
    let new_global = (current.global + damped_global).clamp(GLOBAL_MIN, GLOBAL_MAX);
    let new_track = (current.track + raw_track).clamp(TRACK_MIN, TRACK_MAX);

    AdaptiveUpdate {
        new: Deltas {
            global: new_global,
            track: new_track,
        },
        d_global: new_global - current.global,
        d_track: new_track - current.track,
        applied: true,
        verdict: why,
    }
}

// ─── Testes ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn laps(times: &[f64]) -> Vec<Lap> {
        times
            .iter()
            .enumerate()
            .map(|(i, &t)| Lap {
                lap: i as i32 + 1,
                time: t,
            })
            .collect()
    }

    fn driver(is_player: bool, pos: i32, times: &[f64]) -> DriverData {
        DriverData {
            car_idx: pos,
            is_player,
            is_ai: !is_player,
            car_class_id: 1,
            finish_pos_in_class: pos,
            dnf: false,
            laps: laps(times),
        }
    }

    /// Monta uma corrida: jogador + 2 rivais de IA na mesma classe. A volta 1 de
    /// cada um é "lenta" (largada) e descartada — por isso repito o ritmo.
    fn race(player_times: &[f64], r1: &[f64], r2: &[f64]) -> RaceResult {
        RaceResult {
            track_id: 489,
            yellow_laps: vec![],
            race: vec![
                driver(true, 1, player_times),
                driver(false, 2, r1),
                driver(false, 3, r2),
            ],
            qualy: None,
        }
    }

    // Ritmos: volta 1 inflada (largada) + voltas representativas.
    const LEADER_START: f64 = 110.0;

    fn with_start(rep: &[f64]) -> Vec<f64> {
        let mut v = vec![LEADER_START];
        v.extend_from_slice(rep);
        v
    }

    #[test]
    fn dominou_sobe_forte() {
        // jogador ~95.0, rivais ~96.0 → -1.0s/volta → dominou.
        let r = race(
            &with_start(&[95.0, 95.1, 94.9]),
            &with_start(&[96.0, 96.1, 95.9]),
            &with_start(&[96.0, 96.0, 96.0]),
        );
        let u = compute_adaptive_update(&r, &Deltas::default());
        assert!(u.applied);
        assert_eq!(u.d_global, 2, "{}", u.verdict);
        assert_eq!(u.d_track, 2);
    }

    #[test]
    fn venceu_com_folga_sobe_um() {
        // jogador ~95.5, rivais ~96.0 → -0.5s → folga.
        let r = race(
            &with_start(&[95.5, 95.5, 95.5]),
            &with_start(&[96.0, 96.0, 96.0]),
            &with_start(&[96.0, 96.0, 96.0]),
        );
        let u = compute_adaptive_update(&r, &Deltas::default());
        assert_eq!(u.d_global, 1, "{}", u.verdict);
        assert_eq!(u.d_track, 1);
    }

    #[test]
    fn equilibrio_nao_mexe() {
        // jogador ~95.9, rivais ~96.0 → -0.1s → equilíbrio.
        let r = race(
            &with_start(&[95.9, 95.9, 95.9]),
            &with_start(&[96.0, 96.0, 96.0]),
            &with_start(&[96.0, 96.0, 96.0]),
        );
        let u = compute_adaptive_update(&r, &Deltas::default());
        assert_eq!(u.d_global, 0, "{}", u.verdict);
        assert_eq!(u.d_track, 0);
    }

    #[test]
    fn muito_atras_desce_global_e_pista() {
        // jogador ~97.3, rivais ~96.0 → +1.3s, e best também lento → desce os dois.
        let r = race(
            &with_start(&[97.3, 97.3, 97.3]),
            &with_start(&[96.0, 96.0, 96.0]),
            &with_start(&[96.0, 96.0, 96.0]),
        );
        let u = compute_adaptive_update(&r, &Deltas::default());
        assert_eq!(u.d_global, -1, "{}", u.verdict);
        assert_eq!(u.d_track, -1);
    }

    #[test]
    fn atras_moderado_desce_so_global() {
        // jogador ~96.6, rivais ~96.0 → +0.6s (entre BEHIND e FAR) → -1 global, 0 pista.
        let r = race(
            &with_start(&[96.6, 96.6, 96.6]),
            &with_start(&[96.0, 96.0, 96.0]),
            &with_start(&[96.0, 96.0, 96.0]),
        );
        let u = compute_adaptive_update(&r, &Deltas::default());
        assert_eq!(u.d_global, -1, "{}", u.verdict);
        assert_eq!(u.d_track, 0);
    }

    #[test]
    fn transito_escudo_nao_desce() {
        // Ritmo médio ruim (+1.0s) MAS melhor volta competitiva (best -0.2s vs ref).
        // jogador: voltas lentas por trânsito (97.0) + uma volta limpa boa (95.8).
        // rivais best ~96.0 → best_gap = 95.8 - 96.0 = -0.2 < SHIELD → não desce.
        let r = race(
            &with_start(&[97.0, 97.0, 95.8, 97.0]),
            &with_start(&[96.0, 96.0, 96.0]),
            &with_start(&[96.0, 96.0, 96.0]),
        );
        let u = compute_adaptive_update(&r, &Deltas::default());
        assert_eq!(u.d_global, 0, "escudo do trânsito falhou: {}", u.verdict);
        assert_eq!(u.d_track, 0);
        assert!(u.verdict.contains("Trânsito"));
    }

    #[test]
    fn dnf_nao_ajusta() {
        let mut r = race(
            &with_start(&[95.0, 95.0, 95.0]),
            &with_start(&[96.0, 96.0, 96.0]),
            &with_start(&[96.0, 96.0, 96.0]),
        );
        r.race[0].dnf = true;
        let u = compute_adaptive_update(&r, &Deltas::default());
        assert!(!u.applied);
        assert_eq!(u.d_global, 0);
    }

    #[test]
    fn erro_de_10s_nao_conta_como_lento() {
        // jogador domina (95.0) mas tem UMA volta de 105 (rodada). Não pode parecer lento.
        let r = race(
            &with_start(&[95.0, 95.0, 105.0, 95.0]),
            &with_start(&[96.0, 96.0, 96.0]),
            &with_start(&[96.0, 96.0, 96.0]),
        );
        let u = compute_adaptive_update(&r, &Deltas::default());
        assert_eq!(u.d_global, 2, "outlier não foi descartado: {}", u.verdict);
    }

    #[test]
    fn voltas_amarelas_sao_ignoradas() {
        // jogador rápido (95.0), mas voltas 4-5 são amarelas e lentas — devem sair.
        let mut r = race(
            &[LEADER_START, 95.0, 95.0, 130.0, 130.0],
            &with_start(&[96.0, 96.0, 96.0]),
            &with_start(&[96.0, 96.0, 96.0]),
        );
        r.yellow_laps = vec![4, 5];
        let u = compute_adaptive_update(&r, &Deltas::default());
        assert_eq!(u.d_global, 2, "amarela não foi ignorada: {}", u.verdict);
    }

    #[test]
    fn piso_global_trava_em_menos_cinco() {
        let r = race(
            &with_start(&[98.0, 98.0, 98.0]),
            &with_start(&[96.0, 96.0, 96.0]),
            &with_start(&[96.0, 96.0, 96.0]),
        );
        let u = compute_adaptive_update(
            &r,
            &Deltas {
                global: -5,
                track: 0,
            },
        );
        assert_eq!(u.new.global, -5, "furou o piso: {}", u.verdict);
        assert_eq!(u.d_global, 0);
    }

    #[test]
    fn subida_amortecida_perto_do_teto() {
        // Dominou (+2 cru) mas já está em +12 → amortece p/ +1.
        let r = race(
            &with_start(&[95.0, 95.0, 95.0]),
            &with_start(&[96.0, 96.0, 96.0]),
            &with_start(&[96.0, 96.0, 96.0]),
        );
        let u = compute_adaptive_update(
            &r,
            &Deltas {
                global: 12,
                track: 0,
            },
        );
        assert_eq!(u.d_global, 1, "não amorteceu: {}", u.verdict);
        assert_eq!(u.new.global, 13);
    }

    #[test]
    fn quali_reforca_escudo() {
        // Corrida: ritmo médio ruim (+1.0s) E melhor volta da corrida ruim (96.9),
        // MAS na QUALI o jogador fez 95.7 (rivais quali 96.0) → potencial bom → não desce.
        let mut r = race(
            &with_start(&[97.0, 96.9, 97.0]),
            &with_start(&[96.0, 96.0, 96.0]),
            &with_start(&[96.0, 96.0, 96.0]),
        );
        r.qualy = Some(vec![
            driver(true, 1, &[120.0, 95.7, 95.8]),  // jogador voou na quali
            driver(false, 2, &[120.0, 96.0, 96.1]), // rival
            driver(false, 3, &[120.0, 96.0, 96.0]),
        ]);
        let u = compute_adaptive_update(&r, &Deltas::default());
        assert_eq!(u.d_global, 0, "quali não reforçou o escudo: {}", u.verdict);
    }
}
