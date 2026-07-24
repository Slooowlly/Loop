//! Sinais do **Tier 2** — os mesmos dados do Tier 1 (resultados recentes, idade, fim
//! de temporada) mais o lote B, com sourcing no comando: carreira, contrato, time e
//! companheiro de equipe.

use super::tipos::{fav, Nudge, Signal};

// Tier 2 (mesmos dados de Tier 1: resultados recentes / idade / fim de temporada).
const STREAK_OPT: f64 = 6.0; // por vitória na sequência (cap)
const STREAK_AGG: f64 = 4.0;
const STREAK_CAP: u32 = 3;
const NEARMISS_AGG: f64 = 10.0;
const NEARMISS_OPT: f64 = 8.0;
const FATIGUE_OPT: f64 = 9.0;
const FATIGUE_SMO: f64 = 7.0;
const PRODIGY_OPT: f64 = 10.0;
const PRODIGY_AGG: f64 = 8.0;
// Tier 2 Batch B (sourcing no comando: carreira / contrato / time / companheiro).
const MILESTONE_AGG: f64 = 8.0;
const MILESTONE_OPT: f64 = 8.0;
const CONTRACT_AGG: f64 = 10.0;
const CONTRACT_OPT: f64 = 6.0;
const CONTRACT_SMO: f64 = 12.0;
const TEAMMATE_AGG: f64 = 12.0;
const PROMO_AGG: f64 = 10.0;
const PROMO_SMO: f64 = 12.0;
const RELEG_AGG: f64 = 10.0;
const RELEG_OPT: f64 = 10.0;
const MORALE_OPT: f64 = 12.0;
const MORALE_AGG: f64 = 10.0;
const MORALE_SMO: f64 = 10.0;
const PRIZE_AGG: f64 = 9.0;
const PRIZE_SMO: f64 = 6.0;
const PRIZE_WINDOW: u32 = 3; // só nas últimas corridas
const INJURY_AGG: f64 = 10.0;
const INJURY_OPT: f64 = 8.0;
const INJURY_SMO: f64 = 12.0;

/// Sequência de vitórias (das mais recentes) → swagger. Favorável.
/// `recent_positions` em ordem [mais recente … mais antiga].
pub fn win_streak(recent_positions: &[u32]) -> Signal {
    let streak = recent_positions.iter().take_while(|&&p| p == 1).count() as u32;
    if streak < 2 {
        return Signal::default();
    }
    let n = streak.min(STREAK_CAP) as f64;
    fav(Nudge {
        optimism: n * STREAK_OPT,
        aggression: n * STREAK_AGG,
        ..Default::default()
    })
}

/// Vários pódios SEM vitória → coceira de risco pra finalmente vencer. Favorável (drive).
pub fn near_miss(recent_positions: &[u32]) -> Signal {
    if recent_positions.is_empty() {
        return Signal::default();
    }
    let podiums = recent_positions.iter().filter(|&&p| p <= 3).count();
    let wins = recent_positions.iter().filter(|&&p| p == 1).count();
    if wins > 0 || podiums < 2 {
        return Signal::default();
    }
    let itch = (podiums as f64 / recent_positions.len() as f64).min(1.0);
    fav(Nudge {
        aggression: itch * NEARMISS_AGG,
        optimism: itch * NEARMISS_OPT,
        ..Default::default()
    })
}

/// Desgaste de fim de temporada longa → menos afiado. ADVERSO.
pub fn end_season_fatigue(races_left: u32, season_length: u32) -> Signal {
    if season_length < 8 || races_left > 2 {
        return Signal::default();
    }
    Signal {
        nudge: Nudge {
            optimism: -FATIGUE_OPT,
            smoothness: -FATIGUE_SMO,
            ..Default::default()
        },
        adverse: true,
    }
}

/// Prodígio em ascensão: jovem E indo bem → bola de neve de confiança. Favorável.
pub fn rising_prodigy(age: u32, recent_positions: &[u32], field_size: u32) -> Signal {
    if age > 23 || recent_positions.is_empty() || field_size <= 1 {
        return Signal::default();
    }
    let avg =
        recent_positions.iter().map(|&p| p as f64).sum::<f64>() / recent_positions.len() as f64;
    let depth = ((avg - 1.0) / (field_size as f64 - 1.0)).clamp(0.0, 1.0);
    if depth > 0.4 {
        return Signal::default(); // precisa estar indo BEM
    }
    let youth = ((24.0 - age as f64) / 8.0).clamp(0.0, 1.0);
    let hot = (0.4 - depth) / 0.4; // 0..1
    let k = youth * hot;
    fav(Nudge {
        optimism: k * PRODIGY_OPT,
        aggression: k * PRODIGY_AGG,
        ..Default::default()
    })
}

/// Caça a marco: a PRÓXIMA vitória é um número redondo → fogo extra. Favorável.
pub fn milestone_chase(career_wins: u32) -> Signal {
    let next = career_wins + 1;
    let milestone = next == 10 || next == 25 || (next >= 50 && next % 25 == 0);
    if !milestone {
        return Signal::default();
    }
    fav(Nudge {
        aggression: MILESTONE_AGG,
        optimism: MILESTONE_OPT,
        ..Default::default()
    })
}

/// Último ano de contrato → pressão de impressionar (showboating); frágil fica impreciso.
/// ADVERSO.
pub fn contract_year(last_year: bool, resilience: f64) -> Signal {
    if !last_year {
        return Signal::default();
    }
    let fragile = (0.55 - resilience).max(0.0);
    Signal {
        nudge: Nudge {
            aggression: CONTRACT_AGG, // sempre tenta aparecer
            optimism: CONTRACT_OPT,
            smoothness: -fragile * CONTRACT_SMO, // frágil → ragged
            ..Default::default()
        },
        adverse: true,
    }
}

/// Duelo interno: apanhando do companheiro de equipe → orgulho, responde forte. Favorável.
pub fn teammate_duel(my_points: f64, teammate_points: Option<f64>) -> Signal {
    let Some(tp) = teammate_points else {
        return Signal::default();
    };
    if tp <= my_points {
        return Signal::default();
    }
    let deficit = ((tp - my_points) / 50.0).clamp(0.0, 1.0); // déficit em pontos (cap)
    fav(Nudge {
        aggression: deficit * TEAMMATE_AGG,
        ..Default::default()
    })
}

/// Mudança de categoria: promovido (subiu) → respeito/cautela (ADVERSO); rebaixado
/// (caiu) → swagger (favorável). `mv`: +1 subiu, -1 caiu, 0 nada.
pub fn category_move(mv: i32) -> Signal {
    if mv > 0 {
        Signal {
            nudge: Nudge {
                aggression: -PROMO_AGG,
                smoothness: PROMO_SMO,
                ..Default::default()
            },
            adverse: true,
        }
    } else if mv < 0 {
        fav(Nudge {
            aggression: RELEG_AGG,
            optimism: RELEG_OPT,
            ..Default::default()
        })
    } else {
        Signal::default()
    }
}

/// Moral no time/carro: feliz → calmo e confiante; insatisfeito → frustrado (ADVERSO).
/// `morale` é multiplicador (~0.5 infeliz … 1.5 feliz; 1.0 neutro).
pub fn team_morale(morale: f64) -> Signal {
    let happy = (morale - 1.0).clamp(-1.0, 1.0);
    if happy.abs() < 0.05 {
        return Signal::default();
    }
    if happy > 0.0 {
        fav(Nudge {
            optimism: happy * MORALE_OPT,
            smoothness: happy * MORALE_SMO,
            ..Default::default()
        })
    } else {
        Signal {
            nudge: Nudge {
                aggression: -happy * MORALE_AGG, // infeliz → frustrado/agressivo
                smoothness: happy * MORALE_SMO,  // e mais bruto
                ..Default::default()
            },
            adverse: true,
        }
    }
}

/// Briga por posição/grana no fim: FORA da briga de título, mas com um rival colado
/// numa posição que vale dinheiro → tensão competitiva. ADVERSO.
pub fn prize_fight(
    my_points: f64,
    all_points: &[f64],
    races_left: u32,
    max_points: f64,
    in_title_fight: bool,
) -> Signal {
    if in_title_fight || races_left > PRIZE_WINDOW || all_points.len() < 2 || max_points <= 0.0 {
        return Signal::default();
    }
    let nearest = all_points
        .iter()
        .map(|&p| (p - my_points).abs())
        .filter(|&d| d > 1e-6)
        .fold(f64::MAX, f64::min);
    let reach = races_left as f64 * max_points;
    if nearest == f64::MAX || nearest > reach {
        return Signal::default();
    }
    let tension = (1.0 - nearest / reach).clamp(0.0, 1.0); // rival colado → tensão alta
    Signal {
        nudge: Nudge {
            aggression: tension * PRIZE_AGG,
            smoothness: -tension * PRIZE_SMO,
            ..Default::default()
        },
        adverse: true,
    }
}

/// Retorno de lesão: voltou há poucas corridas → cauteloso, ainda se recompondo.
/// ADVERSO.
pub fn injury_return(recently_returned: bool) -> Signal {
    if !recently_returned {
        return Signal::default();
    }
    Signal {
        nudge: Nudge {
            aggression: -INJURY_AGG,
            optimism: -INJURY_OPT,
            smoothness: INJURY_SMO,
            ..Default::default()
        },
        adverse: true,
    }
}
