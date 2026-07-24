//! Sinais do **Tier 1** — o contexto imediato da corrida: pressão (título e casa
//! cheia), forma, pista, clima, calor, idade, status, corrida em casa e o humor do dia.

use crate::simulation::pressure::{self, TitleContext};
use crate::simulation::track_knowledge::TrackKnowledge;

use super::mentalidade::splitmix;
use super::tipos::{fav, Nudge, Signal};

// --- Magnitudes por sinal (tunáveis; calibrar com teste real) -------------------
const PRESS_AGG: f64 = 10.0;
const PRESS_OPT: f64 = 8.0;
const PRESS_SMO: f64 = 12.0;
const PRESS_SKILL: f64 = 1.9; // pequeno de propósito (pace ~ ±2..4 no extremo)
// Casa cheia (interesse do evento). Neutro mais baixo que o título (0.55) → palco =
// oportunidade, a maioria rende. GAIN leva stakes×forma a uma intensidade ~0..3
// comparável à do título. Reaproveita as magnitudes PRESS_* acima.
const EVENT_PRESS_NEUTRAL: f64 = 0.42;
const EVENT_PRESS_GAIN: f64 = 1.9;
const CRUISE_AGG: f64 = 10.0;
const CRUISE_SMO: f64 = 10.0;
const FORM_OPT: f64 = 16.0;
const FORM_AGG: f64 = 12.0;
const FORM_SMO: f64 = 14.0;
const SURVIVAL_AGG: f64 = 14.0;
const SURVIVAL_SMO: f64 = 12.0;
const AFF_AGG: f64 = 10.0;
const AFF_OPT: f64 = 12.0;
const AFF_SMO: f64 = 12.0;
const RAIN_AGG: f64 = 14.0;
const RAIN_OPT: f64 = 12.0;
const RAIN_SMO: f64 = 16.0;
const HEAT_THRESHOLD: f64 = 28.0;
const HEAT_AGG: f64 = 10.0;
const HEAT_SMO: f64 = 12.0;
const AGE_AGG: f64 = 12.0;
const AGE_OPT: f64 = 10.0;
const AGE_SMO: f64 = 12.0;
const STATUS_OPT: f64 = 12.0;
const STATUS_SMO: f64 = 8.0;
const STATUS_GRID_WEIGHT: f64 = 0.6; // domínio no grid pesa mais que a fama global
const HOME_AGG: f64 = 8.0;
const HOME_OPT: f64 = 12.0;
const WOBBLE: f64 = 6.0;

/// Pressão de campeonato (do topo) + modo cruzeiro. `resilience` (0–1) decide choke
/// (ADVERSO) vs clutch/cruzeiro (favorável).
pub fn pressure_title(title: &TitleContext, races_left: u32, resilience: f64) -> Signal {
    // Cruzeiro: líder com título praticamente decidido → relaxa e administra.
    if title.title_decided && title.is_leader {
        return fav(Nudge {
            aggression: -CRUISE_AGG,
            smoothness: CRUISE_SMO,
            ..Default::default()
        });
    }
    let intensity = pressure::pressure_intensity(title, races_left); // 0..3
    if intensity <= 0.0 {
        return Signal::default();
    }
    let dir = 0.55 - resilience; // >0 = choke (frágil), <0 = clutch (resiliente)
    let nudge = Nudge {
        aggression: intensity * dir * PRESS_AGG, // choke → +agressivo
        optimism: intensity * dir * PRESS_OPT,
        smoothness: -intensity * dir * PRESS_SMO, // choke → −suave (bruto)
        skill: -intensity * dir * PRESS_SKILL,    // choke → pace cai um pouco
    };
    Signal {
        nudge,
        adverse: dir > 0.0,
    } // só o choke é adverso
}

/// Pressão de "casa cheia" (interesse do evento) — UNIVERSAL, ao contrário da de
/// título (que só pega quem briga). Palco = oportunidade: neutro mais baixo (0.42),
/// a maioria rende e só o frágil trava. Pesa MAIS em quem vem em má fase ("algo a
/// provar"). `event_stakes` 0..1 = quanto o evento desperta público. Mesma semântica
/// da sim (simulation/pressure.rs event_pressure_*). Choke = ADVERSO.
pub fn pressure_event(
    event_stakes: f64,
    recent_positions: &[u32],
    field_size: u32,
    resilience: f64,
) -> Signal {
    let stakes = event_stakes.clamp(0.0, 1.0);
    if stakes <= 0.0 {
        return Signal::default();
    }
    // "Algo a provar": quem vem no fundo do grid sente mais (0.7 na frente .. 1.6 no fundo).
    let form_weight = if recent_positions.is_empty() || field_size <= 1 {
        1.0
    } else {
        let avg =
            recent_positions.iter().map(|&p| p as f64).sum::<f64>() / recent_positions.len() as f64;
        let depth = ((avg - 1.0) / (field_size as f64 - 1.0)).clamp(0.0, 1.0);
        0.7 + depth * 0.9
    };
    let intensity = stakes * form_weight * EVENT_PRESS_GAIN; // ~0..3
    let dir = EVENT_PRESS_NEUTRAL - resilience; // >0 choke (só frágil <0.42), <0 clutch
    let nudge = Nudge {
        aggression: intensity * dir * PRESS_AGG,
        optimism: intensity * dir * PRESS_OPT,
        smoothness: -intensity * dir * PRESS_SMO,
        skill: -intensity * dir * PRESS_SKILL,
    };
    Signal {
        nudge,
        adverse: dir > 0.0, // só o choke é adverso (blindável pela compostura)
    }
}

/// Forma/embalo pelos resultados recentes + "seca de resultados" (pressão de baixo).
/// `recent_positions`: posições finais recentes (1 = vitória). Má fase = ADVERSO.
pub fn form(recent_positions: &[u32], field_size: u32, resilience: f64) -> Signal {
    if recent_positions.is_empty() || field_size <= 1 {
        return Signal::default();
    }
    let avg =
        recent_positions.iter().map(|&p| p as f64).sum::<f64>() / recent_positions.len() as f64;
    let depth = ((avg - 1.0) / (field_size as f64 - 1.0)).clamp(0.0, 1.0); // 0 frente .. 1 fundo
    let hot = 0.5 - depth; // >0 em alta, <0 em baixa

    let mut n = Nudge {
        optimism: hot * FORM_OPT,
        aggression: hot.max(0.0) * FORM_AGG, // em alta → mais agressivo
        smoothness: hot.min(0.0) * FORM_SMO, // em baixa → menos suave (abalado)
        skill: 0.0,
    };
    // Seca de resultados (zona de baixo) = pressão de entregar → desespero se frágil.
    if depth > 0.7 {
        let despair = (depth - 0.7) / 0.3; // 0..1
        let choke = (0.55 - resilience).max(0.0); // só o lado frágil
        n.aggression += despair * choke * SURVIVAL_AGG;
        n.smoothness -= despair * choke * SURVIVAL_SMO;
    }
    Signal {
        nudge: n,
        adverse: hot < 0.0,
    } // má fase = adverso
}

/// Afinidade com a pista. Pista nova = ADVERSO (cauteloso); domínio = favorável.
pub fn track_affinity(k: &TrackKnowledge) -> Signal {
    if k.starts == 0 {
        return Signal {
            nudge: Nudge {
                aggression: -AFF_AGG,
                optimism: -AFF_OPT,
                smoothness: AFF_SMO,
                skill: 0.0,
            },
            adverse: true,
        };
    }
    // Domínio = pódio aqui, OU experiente (≥4 largadas) COM um resultado ao menos
    // decente (≤P8). Experiência sem resultado não é mais domínio — abre espaço p/ a
    // pista-fantasma (bogey_track) tratar o experiente-mas-ruim.
    let masters =
        k.best_finish.is_some_and(|b| b <= 3) || (k.starts >= 4 && k.best_finish.is_some_and(|b| b <= 8));
    if masters {
        return fav(Nudge {
            aggression: AFF_AGG,
            optimism: AFF_OPT,
            ..Default::default()
        });
    }
    Signal::default()
}

/// Clima: medo da chuva = ADVERSO (recolhe); mestre = favorável (ataca). `intensity` 0–1.
pub fn weather(is_wet: bool, fator_chuva: f64, intensity: f64) -> Signal {
    if !is_wet {
        return Signal::default();
    }
    let mastery = (fator_chuva.clamp(0.0, 100.0) - 50.0) / 50.0; // -1 teme .. 1 mestre
    let k = intensity.clamp(0.0, 1.0);
    let nudge = Nudge {
        aggression: mastery * k * RAIN_AGG,
        optimism: mastery * k * RAIN_OPT,
        smoothness: -mastery * k * RAIN_SMO, // teme (mastery<0) → suavidade↑
        skill: 0.0,
    };
    Signal {
        nudge,
        adverse: mastery < 0.0,
    } // teme a chuva = adverso
}

/// Calor extremo → gestão de pneu: mais suave, menos agressivo. Traço (não adverso).
pub fn heat(temp_c: f64) -> Signal {
    if temp_c <= HEAT_THRESHOLD {
        return Signal::default();
    }
    let h = ((temp_c - HEAT_THRESHOLD) / 12.0).clamp(0.0, 1.0); // 28..40 °C
    fav(Nudge {
        aggression: -h * HEAT_AGG,
        smoothness: h * HEAT_SMO,
        ..Default::default()
    })
}

/// Idade/fase: jovem cru e afoito; veterano calculista e suave. Traço (não adverso).
pub fn age_phase(age: u32) -> Signal {
    let a = age as f64;
    if a <= 22.0 {
        let y = ((23.0 - a) / 7.0).clamp(0.0, 1.0); // ~16..22
        return fav(Nudge {
            aggression: y * AGE_AGG,
            optimism: y * AGE_OPT,
            smoothness: -y * AGE_SMO,
            skill: 0.0,
        });
    }
    if a >= 33.0 {
        let v = ((a - 32.0) / 8.0).clamp(0.0, 1.0); // 33..40
        return fav(Nudge {
            aggression: -v * AGE_AGG,
            smoothness: v * AGE_SMO,
            ..Default::default()
        });
    }
    Signal::default()
}

/// Status/reputação: fama GLOBAL (40%) + domínio no GRID atual (60%, contexto
/// imediato — alfa de grid fraco > craque anônimo no pelotão). Status baixo = ADVERSO
/// (titubeia); topo = favorável (autoridade). Percentis 0–1 (1 = topo).
pub fn status(global_percentile: f64, grid_percentile: f64) -> Signal {
    let blend = grid_percentile.clamp(0.0, 1.0) * STATUS_GRID_WEIGHT
        + global_percentile.clamp(0.0, 1.0) * (1.0 - STATUS_GRID_WEIGHT);
    let s = (blend - 0.5) * 2.0; // -1 fundo .. 1 topo
    let nudge = Nudge {
        optimism: s * STATUS_OPT,
        smoothness: s.max(0.0) * STATUS_SMO,
        ..Default::default()
    };
    Signal {
        nudge,
        adverse: s < 0.0,
    } // fundo = titubeia (adverso)
}

/// Corrida em casa (país natal == país da pista): motivação extra. Favorável.
pub fn home_race(is_home: bool) -> Signal {
    if !is_home {
        return Signal::default();
    }
    fav(Nudge {
        aggression: HOME_AGG,
        optimism: HOME_OPT,
        ..Default::default()
    })
}

/// "Humor do dia" — ruído pequeno determinístico por (evento, piloto). Sempre vale.
pub fn wobble(seed: u64) -> Signal {
    let r = splitmix(seed);
    let pick = |shift: u32| (((r >> shift) & 0xff) as f64 / 255.0 - 0.5) * 2.0 * WOBBLE;
    fav(Nudge {
        aggression: pick(0),
        optimism: pick(8),
        smoothness: pick(16),
        skill: 0.0,
    })
}
