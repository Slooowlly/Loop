//! Parciais por setor do jogador: melhor por setor, volta teórica e onde ele mais perde.

use serde::Serialize;

use crate::iracing_sdk::race_monitor::RaceHistory;

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
pub(super) fn analyze_sectors(history: &RaceHistory) -> Option<SectorAnalysis> {
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
