//! Estruturas que acompanham uma entidade ao longo das temporadas de uma run:
//! trajetória financeira da equipe, funil de carreira do piloto e a trajetória
//! de habilidade (primeiro vs. último overall).

/// Trajetória financeira de uma equipe ao longo das temporadas de uma run.
#[derive(Default)]
pub(super) struct TeamStateTrack {
    pub(super) ever_collapse: bool,
    pub(super) seasons_in_collapse: u32,
    /// Índice da temporada em que entrou em colapso pela 1ª vez.
    pub(super) first_collapse_season: Option<usize>,
    /// Atingiu "stable" ou melhor DEPOIS de ter colapsado.
    pub(super) recovered: bool,
    /// Saiu do colapso (qualquer estado > collapse) depois de ter colapsado.
    pub(super) escaped: bool,
    /// Temporada em que se recuperou (stable+) pela 1ª vez após colapso.
    pub(super) recover_season: Option<usize>,
    pub(super) final_state_rank: u8,
}

/// Trajetória de carreira por tier de um piloto dentro de uma run.
pub(super) struct CareerTrack {
    pub(super) first_season: usize,
    pub(super) started_rookie: bool,
    pub(super) peak_tier: u8,
    /// Maior habilidade (overall) observada na carreira — proxy de "quão bom ficou".
    pub(super) peak_skill: f64,
    /// Primeira temporada (índice) em que alcançou cada tier.
    pub(super) reached_at: [Option<usize>; 7],
}

/// Trajetória de um piloto dentro de uma run: primeiro e último overall observados.
#[derive(Clone, Copy)]
pub(super) struct Trajectory {
    pub(super) first_overall: f64,
    pub(super) last_overall: f64,
    pub(super) first_age: i32,
    pub(super) seasons_seen: u32,
}
