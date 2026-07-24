//! Horizonte de planejamento: quão longe o time enxerga o calendário ao
//! planejar o carro, sorteado por `(time, temporada)`.

// ===================== Horizonte de planejamento =====================

/// Quão longe o time enxerga o calendário ao planejar o carro. Varia por temporada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanningHorizon {
    /// Míope: só a próxima pista.
    SingleTrack,
    ThreeRaces,
    FiveRaces,
    /// Enxerga a temporada inteira.
    FullSeason,
}

impl PlanningHorizon {
    /// Nº de corridas à frente que o time considera. `None` = temporada inteira.
    pub fn lookahead(self) -> Option<usize> {
        match self {
            PlanningHorizon::SingleTrack => Some(1),
            PlanningHorizon::ThreeRaces => Some(3),
            PlanningHorizon::FiveRaces => Some(5),
            PlanningHorizon::FullSeason => None,
        }
    }
}

/// Horizonte determinístico por `(time, temporada)` — re-rola a cada temporada.
/// Distribuição: 20% míope / 30% 3 corridas / 30% 5 corridas / 20% temporada.
pub fn planning_horizon(team_id: &str, season: i32) -> PlanningHorizon {
    let mut seed: u32 = 0x9E37_79B9;
    for byte in team_id.bytes() {
        seed = seed.wrapping_mul(31).wrapping_add(byte as u32);
    }
    seed = seed
        .wrapping_mul(2_654_435_761)
        .wrapping_add((season as u32).wrapping_mul(40_503));
    // avalanche (mistura bem os bits para o módulo 100 não correlacionar com o input)
    seed ^= seed >> 15;
    seed = seed.wrapping_mul(0x2c1b_3c6d);
    seed ^= seed >> 12;

    match seed % 100 {
        0..=19 => PlanningHorizon::SingleTrack,
        20..=49 => PlanningHorizon::ThreeRaces,
        50..=79 => PlanningHorizon::FiveRaces,
        _ => PlanningHorizon::FullSeason,
    }
}
