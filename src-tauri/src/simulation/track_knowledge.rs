//! Conhecimento de pista → penalidade de skill. **Lógica pura, testável.**
//!
//! Design (travado com o user): pista nova = penalidade MÁXIMA; o piloto aprende
//! correndo (penalidade cai a cada corrida) e zera depois de N corridas OU de um
//! top-5. Só PENALIDADE, nunca bônus — é o que separa rookie de veterano. A mesma
//! penalidade vale na simulação offline E no export pro iRacing (fonte única).
//!
//! - N = 3 em pistas longas (> 3 km), senão 2.
//! - `adaptabilidade` alta aprende rápido (penalidade menor / "domina em menos voltas").
//! - Enferruja: ≥ 2 temporadas sem correr na pista → volta uma penalidade PARCIAL
//!   (metade), que some após correr 1 vez de novo.

use serde::{Deserialize, Serialize};

/// Penalidade máxima (pontos de skill) — pista nova, piloto pouco adaptável.
const MAX_PENALTY: f64 = 8.0;
/// Fração da penalidade ao enferrujar (0.5 × 8 = 4 pontos).
const RUST_FRACTION: f64 = 0.5;
/// Acima disto a pista é "longa" → precisa de 3 corridas pra dominar (senão 2).
const LONG_TRACK_KM: f64 = 3.0;
/// Resultado que "destrava" o conhecimento na hora (top-5).
const TOP_RESULT: u32 = 5;
/// Temporadas sem correr pra começar a enferrujar.
const RUST_SEASONS: i32 = 2;
/// Quanto a adaptabilidade reduz a penalidade (adapt 100 → ×0.3; adapt 0 → ×1.0).
const ADAPT_REDUCTION: f64 = 0.7;

/// O que o piloto sabe de UMA pista (do cache `historico_circuitos`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TrackKnowledge {
    pub starts: u32,
    /// Melhor resultado já obtido na pista (None = nunca terminou/correu).
    pub best_finish: Option<u32>,
    /// Temporada da última corrida na pista (None = nunca correu).
    pub last_season: Option<i32>,
}

/// Penalidade de skill (pontos a SUBTRAIR) por não conhecer a pista.
pub fn track_knowledge_penalty(
    k: &TrackKnowledge,
    track_length_km: f64,
    adaptabilidade: f64,
    current_season: i32,
) -> f64 {
    let races_to_learn: u32 = if track_length_km > LONG_TRACK_KM {
        3
    } else {
        2
    };
    let adapt_scale = (1.0 - adaptabilidade.clamp(0.0, 100.0) / 100.0 * ADAPT_REDUCTION).max(0.0);

    // Pista nova → penalidade máxima.
    if k.starts == 0 {
        return MAX_PENALTY * adapt_scale;
    }

    let learned = k.starts >= races_to_learn || k.best_finish.is_some_and(|b| b <= TOP_RESULT);
    if learned {
        // Enferrujou? (≥ RUST_SEASONS temporadas sem correr aqui).
        let gap = k.last_season.map_or(i32::MAX, |ls| current_season - ls);
        if gap >= RUST_SEASONS {
            return MAX_PENALTY * RUST_FRACTION * adapt_scale;
        }
        return 0.0; // domina a pista
    }

    // Ainda aprendendo: a penalidade cai a cada corrida.
    let progress = k.starts as f64 / races_to_learn as f64;
    MAX_PENALTY * (1.0 - progress) * adapt_scale
}

/// Lê o conhecimento de uma pista do JSON `historico_circuitos` do piloto.
/// Formato: `{ "<track_id>": { "starts", "best_finish", "last_season" } }`.
pub fn from_history(historico: &serde_json::Value, track_id: i64) -> TrackKnowledge {
    let e = historico.get(track_id.to_string());
    TrackKnowledge {
        starts: e
            .and_then(|e| e.get("starts"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32,
        best_finish: e
            .and_then(|e| e.get("best_finish"))
            .and_then(|v| v.as_u64())
            .map(|v| v as u32),
        last_season: e
            .and_then(|e| e.get("last_season"))
            .and_then(|v| v.as_i64())
            .map(|v| v as i32),
    }
}

/// Registra uma corrida na pista no cache (in-place): +1 largada, melhor resultado
/// e temporada da última visita. `finish` = posição final (None se DNF/sem dado).
pub fn record_race(
    historico: &mut serde_json::Value,
    track_id: i64,
    finish: Option<u32>,
    season: i32,
) {
    if !historico.is_object() {
        *historico = serde_json::json!({});
    }
    let cur = from_history(historico, track_id);
    let best = match (cur.best_finish, finish) {
        (Some(b), Some(f)) => Some(b.min(f)),
        (None, f) => f,
        (b, None) => b,
    };
    historico[track_id.to_string()] = serde_json::json!({
        "starts": cur.starts + 1,
        "best_finish": best,
        "last_season": season,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(starts: u32, best: Option<u32>, last: Option<i32>) -> TrackKnowledge {
        TrackKnowledge {
            starts,
            best_finish: best,
            last_season: last,
        }
    }

    #[test]
    fn pista_nova_penalidade_maxima() {
        // 0 largadas, adaptabilidade baixa → 8 pontos.
        assert_eq!(track_knowledge_penalty(&k(0, None, None), 2.5, 0.0, 1), 8.0);
    }

    #[test]
    fn adaptabilidade_reduz() {
        // Adapt 100 → ×0.3 → 2.4; adapt 0 → 8.0.
        let p100 = track_knowledge_penalty(&k(0, None, None), 2.5, 100.0, 1);
        let p0 = track_knowledge_penalty(&k(0, None, None), 2.5, 0.0, 1);
        assert!((p100 - 2.4).abs() < 1e-9, "{p100}");
        assert!(p100 < p0);
    }

    #[test]
    fn aprende_em_2_na_pista_curta() {
        // Pista 2.5 km (curta) → N=2. 1 largada = metade; 2 largadas = aprendeu (0).
        assert_eq!(
            track_knowledge_penalty(&k(1, None, Some(1)), 2.5, 0.0, 1),
            4.0
        );
        assert_eq!(
            track_knowledge_penalty(&k(2, None, Some(1)), 2.5, 0.0, 1),
            0.0
        );
    }

    #[test]
    fn aprende_em_3_na_pista_longa() {
        // Pista 4 km (> 3 km) → N=3. 1 largada ≈ 5.33; 2 ≈ 2.67; 3 = 0.
        let p1 = track_knowledge_penalty(&k(1, None, Some(1)), 4.0, 0.0, 1);
        let p2 = track_knowledge_penalty(&k(2, None, Some(1)), 4.0, 0.0, 1);
        let p3 = track_knowledge_penalty(&k(3, None, Some(1)), 4.0, 0.0, 1);
        assert!((p1 - 16.0 / 3.0).abs() < 1e-9, "{p1}");
        assert!((p2 - 8.0 / 3.0).abs() < 1e-9, "{p2}");
        assert_eq!(p3, 0.0);
    }

    #[test]
    fn top5_destrava_na_hora() {
        // 1 largada só, MAS terminou em 3º → aprendeu (penalidade 0).
        assert_eq!(
            track_knowledge_penalty(&k(1, Some(3), Some(1)), 4.0, 0.0, 1),
            0.0
        );
    }

    #[test]
    fn enferruja_apos_2_temporadas() {
        // Aprendeu (3 largadas) mas última corrida foi na temporada 1, agora é 3
        // (gap 2) → penalidade parcial = 4 (low adapt). Gap 1 = ainda fresco (0).
        assert_eq!(
            track_knowledge_penalty(&k(3, None, Some(1)), 4.0, 0.0, 3),
            4.0
        );
        assert_eq!(
            track_knowledge_penalty(&k(3, None, Some(2)), 4.0, 0.0, 3),
            0.0
        );
    }

    #[test]
    fn record_e_leitura_do_json() {
        let mut h = serde_json::json!({});
        record_race(&mut h, 489, Some(8), 1);
        record_race(&mut h, 489, Some(3), 2); // melhor resultado melhora p/ 3
        let kn = from_history(&h, 489);
        assert_eq!(kn.starts, 2);
        assert_eq!(kn.best_finish, Some(3));
        assert_eq!(kn.last_season, Some(2));
        // Pista nunca corrida → vazio.
        assert_eq!(from_history(&h, 999).starts, 0);
    }
}
