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

/// Piso/teto do delta global. O teto é ALTO (a base dos tiers é achatada, então a
/// dificuldade real mora aqui) — um alien alcança bem acima da base.
const GLOBAL_MIN: i64 = -5;
const GLOBAL_MAX: i64 = 40;

// ─── Regime RÁPIDO (ritmo LIMPO vs a FRENTE) — TODOS os tiers ─────────────────
// Filosofia (A): o alvo é BRIGAR PELA VITÓRIA. A régua é o RITMO da FRENTE = mediana dos 2
// melhores da IA (robusto a 1 outlier), comparada ao ritmo LIMPO do jogador (voltas de
// erro/pit/rodada descartadas) em FRAÇÃO do tempo de volta (imune a duração da prova e
// comprimento da pista). Se você não anda no ritmo da frente, é difícil demais → desce,
// MESMO terminando no meio do grid (posição não salva). Um erro pontual não desce (ritmo
// limpo competitivo = escudo). Mais rápido que a frente → sobe. Delta = `global` (amortecido
// por tier no export). "Mais fácil descer quando está alto" é AUTOMÁTICO: no topo a frente
// é mais rápida, então o jogador cai mais fácil nas faixas de descida.
// Passos (simétricos): subida/descida com patamar BRANDO e FORTE.
/// Muito mais rápido que a frente (dominou) → sobe ISTO.
pub const FAST_UP_DOMINATE: i64 = 5;
/// Mais rápido que a frente, mas < domínio → sobe ISTO.
pub const FAST_UP_CLEAR: i64 = 3;
/// Fora do ritmo da frente (difícil) → desce ISTO.
pub const FAST_DOWN: i64 = 3;
/// Muito fora do ritmo da frente (difícil demais) → desce ISTO.
pub const FAST_DOWN_HARD: i64 = 5;
// Limites de ritmo vs a frente, em fração do tempo de volta. >0 = jogador mais LENTO.
/// Mais rápido que a frente por MAIS que isto (1,0%) → domínio.
pub const FAST_DOMINATE_PCT: f64 = 0.010;
/// Mais rápido que a frente por mais que isto (0,45%) → venceu/andou folgado.
pub const FAST_CLEAR_PCT: f64 = 0.0045;
/// ESCUDO / banda "brigando": ritmo dentro de ±isto (0,4%) da frente = competitivo →
/// mantém. É o que impede um erro pontual (posição ruim, ritmo limpo competitivo) de baixar
/// a dificuldade.
pub const FAST_SHIELD_PCT: f64 = 0.004;
/// Mais lento que a frente por MAIS que isto (1,4%) → difícil demais (desce forte); entre o
/// escudo e isto → difícil (desce brando).
pub const FAST_BEHIND_PCT: f64 = 0.014;

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

/// Resultado da corrida reduzido ao que a regra rápida usa: o RITMO do jogador vs a FRENTE.
#[derive(Clone, Copy, Debug)]
pub struct FastResult {
    /// Posição final do jogador NA CLASSE (só p/ texto do veredito; a decisão é por ritmo).
    pub finish_pos: i32,
    /// Ritmo LIMPO do jogador vs a FRENTE (mediana dos 2 melhores da IA), em FRAÇÃO do tempo
    /// de volta. >0 = mais LENTO que a frente; <0 = mais rápido. `None` = sem voltas limpas
    /// suficientes pra medir → mantém. Erro/pit/rodada NÃO entram (voltas descartadas).
    pub pace_vs_front: Option<f64>,
    pub player_dnf: bool,
}

/// Reduz um [`RaceResult`] ao [`FastResult`]: ritmo LIMPO do jogador vs a FRENTE = mediana
/// dos 2 melhores RITMOS da IA da classe (robusto a 1 outlier). Reusa [`clean_pace`] →
/// voltas de erro/pit/rodada (muito mais lentas que a mediana do piloto) saem da conta.
pub fn fast_result_from(race: &RaceResult) -> FastResult {
    let drivers = &race.race;
    let Some(player) = drivers.iter().find(|d| d.is_player) else {
        return FastResult {
            finish_pos: 0,
            pace_vs_front: None,
            player_dnf: true,
        };
    };
    let class = player.car_class_id;
    let pace = |d: &DriverData| clean_pace(&d.laps, &race.yellow_laps).map(|(typ, _)| typ);
    let player_pace = pace(player);
    // Frente = mediana (média) dos 2 RITMOS LIMPOS mais rápidos da IA da classe.
    let mut ai_paces: Vec<f64> = drivers
        .iter()
        .filter(|d| d.is_ai && d.car_class_id == class)
        .filter_map(pace)
        .collect();
    ai_paces.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let front = match ai_paces.as_slice() {
        [] => None,
        [only] => Some(*only),
        [a, b, ..] => Some((a + b) / 2.0),
    };
    let pace_vs_front = match (player_pace, front) {
        (Some(pp), Some(fr)) if fr > 0.0 => Some((pp - fr) / fr),
        _ => None,
    };
    FastResult {
        finish_pos: player.finish_pos_in_class,
        pace_vs_front,
        player_dnf: player.dnf,
    }
}

/// Regra RÁPIDA (mexe só no `global`). Os gaps viram % DO TEMPO DE VOLTA antes de decidir,
/// pra o veredito não depender da duração da corrida nem do comprimento da pista. SUBIDA
/// (só o líder): dominou (> `FAST_DOMINATE_PCT`) → +forte; venceu folgado (> `FAST_CLEAR_PCT`)
/// → +médio. DESCIDA (2 gatilhos, valem em qualquer posição do pódio): ritmo pro líder pior
/// que `FAST_BEHIND_PCT`, OU terminou FORA da zona segura (gravidade). Dentro da zona e com
/// ritmo → mantém. DNF/sem posição/sem voltas/sem tempo de volta não mexe.
pub fn compute_fast_update(r: &FastResult, current: &Deltas) -> AdaptiveUpdate {
    if r.player_dnf {
        return AdaptiveUpdate::unchanged(current, "DNF → mantém");
    }
    let Some(g) = r.pace_vs_front else {
        return AdaptiveUpdate::unchanged(current, "Sem dados de ritmo → mantém");
    };
    // g = ritmo LIMPO do jogador vs a frente (fração de volta; >0 = mais lento). A régua é
    // BRIGAR PELA VITÓRIA: perto da frente (±escudo) = mantém; mais rápido = sobe; mais
    // lento = desce, mesmo terminando no meio do grid (a posição não salva). Como é ritmo
    // limpo, um erro pontual (que só afundou a posição) não conta.
    let pc = g * 100.0;
    let p = r.finish_pos; // só pro texto (debug); a decisão é 100% por ritmo.
    let (d, verdict) = if g <= -FAST_DOMINATE_PCT {
        (FAST_UP_DOMINATE, format!("Dominou a frente (P{p}, {pc:+.1}%/volta) → sobe forte"))
    } else if g <= -FAST_CLEAR_PCT {
        (FAST_UP_CLEAR, format!("Mais rápido que a frente (P{p}, {pc:+.1}%/volta) → sobe"))
    } else if g <= FAST_SHIELD_PCT {
        (0, format!("Brigando com a frente (P{p}, {pc:+.1}%/volta) → mantém"))
    } else if g <= FAST_BEHIND_PCT {
        (-FAST_DOWN, format!("Fora do ritmo da frente (P{p}, {pc:+.1}%/volta) → desce"))
    } else {
        (-FAST_DOWN_HARD, format!("Sem ritmo pra frente (P{p}, {pc:+.1}%/volta) → desce forte"))
    };
    let new_global = (current.global + d).clamp(GLOBAL_MIN, GLOBAL_MAX);
    let d_global = new_global - current.global;
    AdaptiveUpdate {
        new: Deltas {
            global: new_global,
            track: current.track,
        },
        d_global,
        d_track: 0,
        applied: d_global != 0,
        verdict,
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

// NOTA: o núcleo por-ritmo antigo (`compute_adaptive_update` + `class_reference` +
// `clean_best` + `class_qualy_best`) foi REMOVIDO — a regra rápida (`compute_fast_update`)
// absorveu a ideia (ritmo LIMPO do jogador vs a frente) de forma mais direta. `clean_pace`
// e `median` ficaram por serem reusados por ela.

// ─── Testes ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Só o ritmo vs a frente importa (fração de volta; >0 = mais lento). finish_pos é só
    /// texto. field_size sumiu (a decisão não é mais por posição).
    fn fast(pace_vs_front: f64) -> FastResult {
        FastResult {
            finish_pos: 5,
            pace_vs_front: Some(pace_vs_front),
            player_dnf: false,
        }
    }

    #[test]
    fn ritmo_vs_frente_decide_os_5_patamares() {
        let cur = Deltas { global: 0, track: 0 };
        let d = |g: f64| compute_fast_update(&fast(g), &cur).d_global;
        assert_eq!(d(-0.015), FAST_UP_DOMINATE); // 1,5% mais rápido → domina → +5
        assert_eq!(d(-0.006), FAST_UP_CLEAR); //     0,6% mais rápido → folgado → +3
        assert_eq!(d(-0.002), 0); //                 0,2% mais rápido (dentro do escudo) → mantém
        assert_eq!(d(0.003), 0); //                  0,3% mais lento (brigando) → mantém
        assert_eq!(d(0.008), -FAST_DOWN); //         0,8% mais lento → difícil → −3
        assert_eq!(d(0.02), -FAST_DOWN_HARD); //     2% mais lento → difícil demais → −5
    }

    #[test]
    fn erro_pontual_nao_baixa_a_dificuldade() {
        // Caso real do molde: rodou, terminou P7, MAS o ritmo LIMPO vs a frente era
        // competitivo (0,3%, dentro do escudo). Filosofia A + ritmo limpo → mantém, NÃO
        // baixa. A posição P7 (custo do erro) é ignorada pela decisão.
        let cur = Deltas { global: 30, track: 0 };
        assert_eq!(
            compute_fast_update(&FastResult { finish_pos: 7, ..fast(0.003) }, &cur).d_global,
            0
        );
        // Contraste: genuinamente sem ritmo pra frente (1,8%) → desce forte, mesmo em P4.
        assert_eq!(
            compute_fast_update(&FastResult { finish_pos: 4, ..fast(0.018) }, &cur).d_global,
            -FAST_DOWN_HARD
        );
    }

    #[test]
    fn frente_e_a_mediana_dos_2_melhores_da_ia() {
        // Jogador ~70,3s (com uma rodada de 95 descartada). Duas IA da frente a 70,0 e 70,2
        // → frente = 70,1. Um 3º carro lento (73) NÃO puxa a frente. Déficit ≈ 0,2/70 ≈ 0,3%.
        let race = RaceResult {
            track_id: 1,
            yellow_laps: vec![],
            race: vec![
                driver(false, 1, &[70.0, 70.0, 70.0, 70.0, 70.0]), // frente 1
                driver(false, 2, &[70.2, 70.2, 70.2, 70.2, 70.2]), // frente 2
                driver(false, 3, &[73.0, 73.0, 73.0, 73.0, 73.0]), // IA lenta (fora da frente)
                DriverData {
                    finish_pos_in_class: 7,
                    ..driver(true, 4, &[70.3, 70.3, 95.0, 70.3, 70.3]) // jogador + rodada
                },
            ],
            qualy: None,
        };
        let r = fast_result_from(&race);
        let g = r.pace_vs_front.unwrap();
        assert!((g - 0.00285).abs() < 0.001, "déficit vs frente {g} (~0,3%)");
        // Competitivo (< escudo) → não desce, apesar do P7.
        assert_eq!(compute_fast_update(&r, &Deltas { global: 30, track: 0 }).d_global, 0);
    }

    #[test]
    fn fast_dnf_e_sem_dados_e_teto() {
        let cur = Deltas { global: 0, track: 0 };
        assert!(!compute_fast_update(&FastResult { player_dnf: true, ..fast(-0.02) }, &cur).applied);
        assert!(
            !compute_fast_update(
                &FastResult { pace_vs_front: None, ..fast(0.0) },
                &cur
            )
            .applied
        );
        // No teto global (+40), domínio não passa do limite.
        let topo = Deltas { global: GLOBAL_MAX, track: 0 };
        let u = compute_fast_update(&fast(-0.02), &topo);
        assert_eq!(u.new.global, GLOBAL_MAX);
        assert_eq!(u.d_global, 0);
    }

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

}
