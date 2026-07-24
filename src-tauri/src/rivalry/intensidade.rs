//! Níveis semânticos de intensidade percebida e cruzamento de thresholds
//! (extraído de `rivalry/mod.rs`).

// ── Constantes de domínio ─────────────────────────────────────────────────────

pub(crate) const AXIS_MAX: f64 = 100.0;
pub(crate) const AXIS_MIN: f64 = 0.0;

// ── Passo 9: Thresholds semânticos (sobre intensidade percebida) ──────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RivalryIntensityLevel {
    AtritoLeve, // 0–19
    Inicial,    // 20–39
    Clara,      // 40–59
    Forte,      // 60–79
    Intensa,    // 80–100
}

impl RivalryIntensityLevel {
    pub fn label(&self) -> String {
        let key = match self {
            RivalryIntensityLevel::AtritoLeve => "rivalry.intensity.atrito_leve",
            RivalryIntensityLevel::Inicial => "rivalry.intensity.inicial",
            RivalryIntensityLevel::Clara => "rivalry.intensity.clara",
            RivalryIntensityLevel::Forte => "rivalry.intensity.forte",
            RivalryIntensityLevel::Intensa => "rivalry.intensity.intensa",
        };
        rust_i18n::t!(key).to_string()
    }
}

pub fn intensity_level(v: f64) -> RivalryIntensityLevel {
    if v < 20.0 {
        RivalryIntensityLevel::AtritoLeve
    } else if v < 40.0 {
        RivalryIntensityLevel::Inicial
    } else if v < 60.0 {
        RivalryIntensityLevel::Clara
    } else if v < 80.0 {
        RivalryIntensityLevel::Forte
    } else {
        RivalryIntensityLevel::Intensa
    }
}

/// Retorna o nível mais alto cruzado *para cima* ao passar de `old` para `new` (percebida).
/// Retorna `None` se nenhum threshold foi cruzado. `pub(crate)` para reuso pelo gêmeo de
/// rivalidade entre equipes (`rivalry::team`) — os thresholds são os mesmos.
pub(crate) fn crossed_threshold(old: f64, new: f64) -> Option<RivalryIntensityLevel> {
    if new <= old {
        return None;
    }
    const THRESHOLDS: [(f64, RivalryIntensityLevel); 4] = [
        (20.0, RivalryIntensityLevel::Inicial),
        (40.0, RivalryIntensityLevel::Clara),
        (60.0, RivalryIntensityLevel::Forte),
        (80.0, RivalryIntensityLevel::Intensa),
    ];
    let mut highest: Option<RivalryIntensityLevel> = None;
    for (threshold, level) in &THRESHOLDS {
        if old < *threshold && new >= *threshold {
            highest = Some(level.clone());
        }
    }
    highest
}

// ── Helpers ───────────────────────────────────────────────────────────────────

pub(crate) fn clamp(v: f64) -> f64 {
    v.clamp(AXIS_MIN, AXIS_MAX)
}
