//! Composição: soma os nudges a partir da base, escala pela maleabilidade e clampa —
//! e a entrada única do export (`compute`), que monta todos os sinais e blinda o adverso.

use crate::simulation::pressure;

use super::mentalidade::{adverse_multiplier, malleability};
use super::sinais_tier1::*;
use super::sinais_tier2::*;
use super::sinais_tier3::*;
use super::tipos::{BehaviorInputs, BehaviorOutput, Nudge};

/// Soma os nudges a partir da base, escala pela maleabilidade e clampa 0–100.
pub fn compose(
    base_aggression: f64,
    base_optimism: f64,
    base_smoothness: f64,
    base_skill: f64,
    mentality: f64,
    nudges: &[Nudge],
) -> BehaviorOutput {
    let gain = malleability(mentality);
    let sum = nudges.iter().copied().fold(Nudge::default(), Nudge::add);
    // Headroom: o nudge de pace (essencialmente da pressão) vira pontos de skill
    // conforme onde o piloto está na curva — subir tem teto, cair tem chão. Mesma
    // curva da sim (simulation/pressure.rs).
    let skill_nudge = sum.skill * gain;
    let hr = pressure::headroom_pace_mult(base_skill, skill_nudge >= 0.0);
    BehaviorOutput {
        aggression: (base_aggression + sum.aggression * gain).clamp(0.0, 100.0),
        optimism: (base_optimism + sum.optimism * gain).clamp(0.0, 100.0),
        smoothness: (base_smoothness + sum.smoothness * gain).clamp(0.0, 100.0),
        // Pace: a base já entra com a penalidade de pista; aqui só o nudge, com headroom.
        skill: (base_skill + skill_nudge * hr).clamp(0.0, 100.0),
    }
}

/// Entrada única do export: monta os sinais do Tier 1, blinda o adverso se o piloto
/// passar no teste de compostura, e compõe.
pub fn compute(i: &BehaviorInputs) -> BehaviorOutput {
    let amult = adverse_multiplier(i.mentality, i.seed);
    let signals = [
        pressure_title(&i.title, i.races_left, i.resilience),
        pressure_event(
            i.event_stakes,
            &i.recent_positions,
            i.field_size,
            i.resilience,
        ),
        form(&i.recent_positions, i.field_size, i.resilience),
        track_affinity(&i.track),
        weather(i.is_wet, i.fator_chuva, i.rain_intensity),
        heat(i.temp_c),
        age_phase(i.age),
        status(i.global_rank_percentile, i.grid_rank_percentile),
        home_race(i.home_race),
        win_streak(&i.recent_positions),
        near_miss(&i.recent_positions),
        end_season_fatigue(i.races_left, i.season_length),
        rising_prodigy(i.age, &i.recent_positions, i.field_size),
        milestone_chase(i.career_wins),
        contract_year(i.contract_last_year, i.resilience),
        teammate_duel(i.season_points, i.teammate_points),
        category_move(i.category_move),
        team_morale(i.team_morale),
        prize_fight(
            i.season_points,
            &i.all_points,
            i.races_left,
            i.max_points,
            i.title.in_contention,
        ),
        injury_return(i.injury_return),
        honeymoon(i.honeymoon),
        revenge(i.crashed_out_last_race),
        bad_luck(i.not_at_fault_dnfs),
        track_trauma(i.track_crash),
        nemesis(i.nemesis),
        former_team_grudge(i.switched_teams),
        reigning_champion(i.reigning_champion, i.resilience),
        career_debut(i.career_debut),
        mechanical_distrust(i.mechanical_dnfs),
        bogey_track(&i.track, i.field_size),
        stardom(i.fame),
        team_bond(i.bond_level),
        wobble(i.seed),
    ];
    let nudges: Vec<Nudge> = signals
        .iter()
        .map(|s| {
            if s.adverse {
                s.nudge.scale(amult)
            } else {
                s.nudge
            }
        })
        .collect();
    // Handicap de lesão ATIVA: o machucado CORRE, mas com o pace reduzido (mesma rampa da
    // sim: skill × penalidade × recuperação, que diminui a cada etapa até sarar). É físico,
    // não psicológico → entra direto no pace base, sem passar pela maleabilidade/compostura.
    let injured_skill = i.base_skill * (1.0 - i.injury_active_penalty.clamp(0.0, 1.0));
    compose(
        i.base_aggression,
        i.base_optimism,
        i.base_smoothness,
        injured_skill,
        i.mentality,
        &nudges,
    )
}
