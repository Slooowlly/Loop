use crate::simulation::track_profile::TrackCharacter;

// ---------------------------------------------------------------------------
// Dificuldade de pista (mapa estático para pistas com identidade reconhecida)
// ---------------------------------------------------------------------------

pub(super) fn track_difficulty_for(track_id: u32) -> f64 {
    match track_id {
        249 => 1.6, // Nordschleife
        219 => 1.5, // Mount Panorama / Bathurst
        252 => 1.5, // Nürburgring Combined 24h
        523 => 1.4, // Spa
        527 => 1.3, // Cadwell Park
        168 => 1.3, // Suzuka
        413 => 1.2, // Hungaroring (técnico/lento)
        268 => 1.2, // Le Mans (longo e exigente)
        554 => 0.9, // Charlotte Roval (mais fácil de ultrapassar)
        192 => 0.9, // Daytona Road (roval)
        // ── Venues novos (pesquisados) ──
        405 => 1.45, // Chicago Street (90°, muros, baixa aderência)
        580 => 1.35, // Adelaide (street, muros)
        589 => 1.35, // Coronado (airbase, muros de concreto)
        584 => 1.35, // St. Petersburg (bumpy, muros)
        218 => 1.30, // Gilles Villeneuve (Wall of Champions)
        521 => 1.30, // Sachsenring (waterfall cego, Omega)
        481 => 1.30, // Willow Springs (sweepers rápidos, cegos)
        539 => 1.25, // Miami (setor técnico murado)
        585 => 1.20, // Barber (ápices cegos, elevação)
        423 => 1.20, // Knockhill (ondulado, cego)
        475 => 1.15, // Aragón GP (esses cegos, comprometido)
        476 => 1.12, // Aragón National
        473 => 1.10, // Jerez
        _ => 1.0,    // baseline
    }
}

pub(super) fn overtaking_difficulty_for(character: TrackCharacter) -> f64 {
    match character {
        TrackCharacter::Roval => 0.80,
        TrackCharacter::Flowing => 0.90,
        TrackCharacter::Technical => 1.00,
        TrackCharacter::Tight => 1.15,
    }
}
