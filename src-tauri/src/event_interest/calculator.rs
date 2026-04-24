use crate::models::enums::{SeasonPhase, ThematicSlot};

use super::models::{
    EventInterestContext, EventInterestSummary, ExpectedEventInterest, HeadlineStrength,
    InterestTier, RealizedEventInterest,
};

// ── Cálculo principal ─────────────────────────────────────────────────────────

pub fn calculate_expected_event_interest(ctx: &EventInterestContext) -> ExpectedEventInterest {
    let score = base_score_for_category(&ctx.categoria)
        + phase_bonus(ctx.season_phase)
        + round_importance_bonus(ctx.rodada, ctx.total_rodadas)
        + thematic_slot_bonus(ctx.thematic_slot)
        + competitive_context_bonus(ctx)
        + player_prominence_bonus(ctx);

    let display_value = (score * 450.0).round() as i32;
    let tier = score_to_tier(score);
    let pressure_modifier = 1.0 + (score / 100.0) * 0.20;
    let media_multiplier = 1.0 + (score / 100.0) * 0.35;
    let motivation_multiplier = 1.0 + (score / 100.0) * 0.25;

    ExpectedEventInterest {
        score,
        display_value,
        tier,
        pressure_modifier,
        media_multiplier,
        motivation_multiplier,
    }
}

// ── Utilitários públicos ──────────────────────────────────────────────────────

pub fn to_summary(result: &ExpectedEventInterest) -> EventInterestSummary {
    EventInterestSummary {
        display_value: result.display_value,
        tier: result.tier.clone(),
        tier_label: tier_label(&result.tier).to_string(),
    }
}

pub fn tier_label(tier: &InterestTier) -> &'static str {
    match tier {
        InterestTier::Baixo => "Interesse baixo",
        InterestTier::Moderado => "Interesse moderado",
        InterestTier::Alto => "Grande público",
        InterestTier::MuitoAlto => "Evento de destaque",
        InterestTier::EventoPrincipal => "Evento principal",
    }
}

// ── Blocos internos do score ──────────────────────────────────────────────────

fn base_score_for_category(categoria: &str) -> f32 {
    match categoria {
        "mazda_rookie" | "toyota_rookie" => 18.0,
        "mazda_amador" | "toyota_amador" => 28.0,
        "bmw_m2" => 40.0,
        "gt4" => 52.0,
        "production_challenger" => 62.0,
        "gt3" => 68.0,
        "endurance" => 82.0,
        _ => 30.0,
    }
}

/// Bônus aditivo pelo papel narrativo da corrida.
/// Opera em paralelo com round_importance_bonus (não o substitui).
/// NaoClassificado e slots regulares recebem 0 — compatibilidade com saves legados.
fn thematic_slot_bonus(slot: ThematicSlot) -> f32 {
    match slot {
        ThematicSlot::AberturaDaTemporada => 4.0,
        ThematicSlot::FinalDaTemporada => 6.0,
        ThematicSlot::TensaoPreFinal => 4.0,
        ThematicSlot::MidpointPrestigio => 3.0,
        ThematicSlot::VisitanteRegional => 2.0,
        ThematicSlot::AberturaEspecial => 3.0,
        ThematicSlot::FinalEspecial => 7.0,
        ThematicSlot::RodadaRegular
        | ThematicSlot::RodadaEspecial
        | ThematicSlot::NaoClassificado => 0.0,
    }
}

fn phase_bonus(phase: SeasonPhase) -> f32 {
    match phase {
        SeasonPhase::BlocoEspecial => 10.0,
        _ => 0.0,
    }
}

fn round_importance_bonus(rodada: i32, total_rodadas: i32) -> f32 {
    if total_rodadas <= 0 {
        return 0.0;
    }
    if rodada == 1 {
        return 6.0;
    }
    if rodada == total_rodadas {
        return 12.0;
    }
    if rodada == total_rodadas - 1 {
        return 8.0;
    }
    let progress = rodada as f32 / total_rodadas as f32;
    if progress > 0.5 {
        2.0
    } else {
        0.0
    }
}

fn competitive_context_bonus(ctx: &EventInterestContext) -> f32 {
    let mut bonus = 0.0_f32;
    if ctx.is_title_decider_candidate {
        bonus += 10.0;
    }
    if let Some(gap) = ctx.championship_gap_to_leader {
        if gap <= 10 {
            bonus += 6.0;
        } else if gap <= 20 {
            bonus += 3.0;
        }
    }
    bonus
}

fn player_prominence_bonus(ctx: &EventInterestContext) -> f32 {
    if !ctx.is_player_event {
        return 0.0;
    }
    let mut bonus = 0.0_f32;
    if let Some(pos) = ctx.player_championship_position {
        bonus += match pos {
            1..=3 => 8.0,
            4..=5 => 5.0,
            6..=10 => 2.0,
            _ => 0.0,
        };
    }
    if let Some(media) = ctx.player_media {
        if media >= 80.0 {
            bonus += 5.0;
        } else if media >= 65.0 {
            bonus += 3.0;
        }
    }
    bonus
}

fn score_to_tier(score: f32) -> InterestTier {
    if score >= 85.0 {
        InterestTier::EventoPrincipal
    } else if score >= 65.0 {
        InterestTier::MuitoAlto
    } else if score >= 45.0 {
        InterestTier::Alto
    } else if score >= 25.0 {
        InterestTier::Moderado
    } else {
        InterestTier::Baixo
    }
}

// ── Cálculo de repercussão pós-corrida ────────────────────────────────────────

pub fn calculate_realized_event_interest(
    expected: &ExpectedEventInterest,
    ctx: &EventInterestContext,
    finish_position: Option<i32>,
    grid_position: Option<i32>,
    player_won: bool,
    player_podium: bool,
    player_dnf: bool,
    is_final_round_decider: bool,
) -> RealizedEventInterest {
    let result_bonus = if player_won {
        10.0
    } else if player_podium {
        6.0
    } else if finish_position.map_or(false, |p| p <= 5) {
        3.0
    } else if player_dnf {
        -8.0
    } else if finish_position.map_or(false, |p| p > 10) {
        -2.0
    } else {
        0.0
    };

    let positions_gained = match (finish_position, grid_position) {
        (Some(f), Some(g)) => g - f,
        _ => 0,
    };
    let positional_bonus = if positions_gained >= 5 {
        4.0
    } else if positions_gained >= 2 {
        2.0
    } else if positions_gained <= -5 {
        -3.0
    } else {
        0.0
    };

    let big_event_bonus = if (expected.tier == InterestTier::MuitoAlto
        || expected.tier == InterestTier::EventoPrincipal)
        && (player_won || player_podium)
    {
        5.0
    } else {
        0.0
    };

    let title_bonus = if is_final_round_decider {
        8.0
    } else if ctx.is_title_decider_candidate {
        5.0
    } else {
        0.0
    };

    let final_score =
        (expected.score + result_bonus + positional_bonus + big_event_bonus + title_bonus)
            .clamp(0.0, 120.0);

    let media_delta_modifier = (0.75 + (final_score / 100.0) * 0.85).clamp(0.75, 1.60);
    let motivation_delta_modifier = (0.85 + (final_score / 100.0) * 0.65).clamp(0.85, 1.50);

    let news_importance_bias = if final_score >= 85.0 {
        2
    } else if final_score >= 55.0 {
        1
    } else {
        0
    };

    let headline_strength = match news_importance_bias {
        2 => HeadlineStrength::Principal,
        1 => HeadlineStrength::Forte,
        _ => HeadlineStrength::Normal,
    };

    RealizedEventInterest {
        expected_display_value: expected.display_value,
        expected_tier: expected.tier.clone(),
        final_score,
        final_display_value: (final_score * 450.0).round() as i32,
        final_tier: score_to_tier(final_score),
        delta_vs_expected: final_score - expected.score,
        media_delta_modifier,
        motivation_delta_modifier,
        news_importance_bias,
        headline_strength,
    }
}

// ── Testes ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_interest::models::EventInterestContext;
    use crate::models::enums::ThematicSlot;

    fn base_ctx(categoria: &str) -> EventInterestContext {
        EventInterestContext {
            categoria: categoria.to_string(),
            season_phase: SeasonPhase::BlocoRegular,
            rodada: 5,
            total_rodadas: 14,
            week_of_year: 20,
            track_id: 1,
            track_name: "Spa-Francorchamps".to_string(),
            is_player_event: false,
            player_championship_position: None,
            player_media: None,
            championship_gap_to_leader: None,
            is_title_decider_candidate: false,
            thematic_slot: ThematicSlot::NaoClassificado,
        }
    }

    // ── Testes de categoria ───────────────────────────────────────────────────

    #[test]
    fn endurance_maior_que_gt3() {
        let endurance = calculate_expected_event_interest(&base_ctx("endurance"));
        let gt3 = calculate_expected_event_interest(&base_ctx("gt3"));
        assert!(endurance.score > gt3.score);
    }

    #[test]
    fn gt3_maior_que_gt4() {
        let gt3 = calculate_expected_event_interest(&base_ctx("gt3"));
        let gt4 = calculate_expected_event_interest(&base_ctx("gt4"));
        assert!(gt3.score > gt4.score);
    }

    #[test]
    fn gt4_maior_que_bmw_m2() {
        let gt4 = calculate_expected_event_interest(&base_ctx("gt4"));
        let bmw = calculate_expected_event_interest(&base_ctx("bmw_m2"));
        assert!(gt4.score > bmw.score);
    }

    // ── Testes de fase ────────────────────────────────────────────────────────

    #[test]
    fn bloco_especial_maior_que_bloco_regular() {
        let mut ctx_regular = base_ctx("gt3");
        let mut ctx_especial = base_ctx("gt3");
        ctx_regular.season_phase = SeasonPhase::BlocoRegular;
        ctx_especial.season_phase = SeasonPhase::BlocoEspecial;
        let regular = calculate_expected_event_interest(&ctx_regular);
        let especial = calculate_expected_event_interest(&ctx_especial);
        assert!(especial.score > regular.score);
    }

    // ── Testes de rodada ──────────────────────────────────────────────────────

    #[test]
    fn ultima_rodada_maior_que_intermediaria() {
        let mut ctx_final = base_ctx("gt3");
        let mut ctx_meio = base_ctx("gt3");
        ctx_final.rodada = 14;
        ctx_meio.rodada = 7;
        let final_result = calculate_expected_event_interest(&ctx_final);
        let meio_result = calculate_expected_event_interest(&ctx_meio);
        assert!(final_result.score > meio_result.score);
    }

    #[test]
    fn abertura_maior_que_intermediaria() {
        let mut ctx_abertura = base_ctx("gt3");
        let mut ctx_meio = base_ctx("gt3");
        ctx_abertura.rodada = 1;
        ctx_meio.rodada = 7;
        let abertura = calculate_expected_event_interest(&ctx_abertura);
        let meio = calculate_expected_event_interest(&ctx_meio);
        assert!(abertura.score > meio.score);
    }

    // ── Testes de campeonato ──────────────────────────────────────────────────

    #[test]
    fn title_decider_aumenta_score() {
        let mut ctx_normal = base_ctx("gt3");
        let mut ctx_decisivo = base_ctx("gt3");
        ctx_normal.is_title_decider_candidate = false;
        ctx_decisivo.is_title_decider_candidate = true;
        let normal = calculate_expected_event_interest(&ctx_normal);
        let decisivo = calculate_expected_event_interest(&ctx_decisivo);
        assert!(decisivo.score > normal.score);
    }

    #[test]
    fn gap_pequeno_aumenta_score() {
        let mut ctx_longe = base_ctx("gt3");
        let mut ctx_perto = base_ctx("gt3");
        ctx_longe.championship_gap_to_leader = Some(50);
        ctx_perto.championship_gap_to_leader = Some(8);
        let longe = calculate_expected_event_interest(&ctx_longe);
        let perto = calculate_expected_event_interest(&ctx_perto);
        assert!(perto.score > longe.score);
    }

    // ── Testes de protagonismo do jogador ─────────────────────────────────────

    #[test]
    fn jogador_top3_com_midia_alta_maior_que_sem_destaque() {
        let mut ctx_destaque = base_ctx("gt3");
        ctx_destaque.is_player_event = true;
        ctx_destaque.player_championship_position = Some(2);
        ctx_destaque.player_media = Some(85.0);

        let mut ctx_sem = base_ctx("gt3");
        ctx_sem.is_player_event = true;
        ctx_sem.player_championship_position = Some(15);
        ctx_sem.player_media = Some(40.0);

        let destaque = calculate_expected_event_interest(&ctx_destaque);
        let sem = calculate_expected_event_interest(&ctx_sem);
        assert!(destaque.score > sem.score);
    }

    // ── Testes de tier ────────────────────────────────────────────────────────

    #[test]
    fn rookie_miolo_temporada_cai_em_baixo_ou_moderado() {
        let mut ctx = base_ctx("mazda_rookie");
        ctx.rodada = 5;
        ctx.total_rodadas = 14;
        let result = calculate_expected_event_interest(&ctx);
        assert!(
            result.tier == InterestTier::Baixo || result.tier == InterestTier::Moderado,
            "Esperado Baixo ou Moderado, mas foi {:?} (score={})",
            result.tier,
            result.score
        );
    }

    #[test]
    fn endurance_bloco_especial_title_decider_cai_em_evento_principal() {
        let mut ctx = base_ctx("endurance");
        ctx.season_phase = SeasonPhase::BlocoEspecial;
        ctx.is_title_decider_candidate = true;
        ctx.championship_gap_to_leader = Some(5);
        let result = calculate_expected_event_interest(&ctx);
        assert_eq!(
            result.tier,
            InterestTier::EventoPrincipal,
            "Score={}, esperado EventoPrincipal",
            result.score
        );
    }

    #[test]
    fn display_value_cresce_com_score() {
        let rookie = calculate_expected_event_interest(&base_ctx("mazda_rookie"));
        let gt3 = calculate_expected_event_interest(&base_ctx("gt3"));
        let endurance = calculate_expected_event_interest(&base_ctx("endurance"));
        assert!(gt3.display_value > rookie.display_value);
        assert!(endurance.display_value > gt3.display_value);
    }

    // ── Helpers para testes de repercussão ───────────────────────────────────

    fn realized_ctx(categoria: &str) -> (ExpectedEventInterest, EventInterestContext) {
        let ctx = base_ctx(categoria);
        let expected = calculate_expected_event_interest(&ctx);
        (expected, ctx)
    }

    fn realized_with(
        categoria: &str,
        finish: i32,
        grid: i32,
        won: bool,
        podium: bool,
        dnf: bool,
        final_decider: bool,
    ) -> RealizedEventInterest {
        let (expected, ctx) = realized_ctx(categoria);
        calculate_realized_event_interest(
            &expected,
            &ctx,
            Some(finish),
            Some(grid),
            won,
            podium,
            dnf,
            final_decider,
        )
    }

    // ── Testes de repercussão — resultado ────────────────────────────────────

    #[test]
    fn vitoria_aumenta_score_final() {
        let vitoria = realized_with("gt3", 1, 3, true, true, false, false);
        let decimo = realized_with("gt3", 10, 10, false, false, false, false);
        assert!(vitoria.final_score > decimo.final_score);
    }

    #[test]
    fn dnf_reduz_score_final() {
        let normal = realized_with("gt3", 8, 8, false, false, false, false);
        let dnf = realized_with("gt3", 20, 5, false, false, true, false);
        assert!(dnf.final_score < normal.final_score);
    }

    #[test]
    fn podio_maior_que_resultado_medio() {
        let podio = realized_with("gt3", 3, 5, false, true, false, false);
        let medio = realized_with("gt3", 8, 8, false, false, false, false);
        assert!(podio.final_score > medio.final_score);
    }

    // ── Testes de repercussão — contexto ────────────────────────────────────

    #[test]
    fn final_decider_aumenta_repercussao() {
        let mut ctx_decider = base_ctx("gt3");
        ctx_decider.rodada = ctx_decider.total_rodadas;
        ctx_decider.is_title_decider_candidate = true;
        let expected_decider = calculate_expected_event_interest(&ctx_decider);
        let com = calculate_realized_event_interest(
            &expected_decider,
            &ctx_decider,
            Some(1),
            Some(3),
            true,
            true,
            false,
            true,
        );

        let ctx_normal = base_ctx("gt3");
        let expected_normal = calculate_expected_event_interest(&ctx_normal);
        let sem = calculate_realized_event_interest(
            &expected_normal,
            &ctx_normal,
            Some(1),
            Some(3),
            true,
            true,
            false,
            false,
        );
        assert!(com.final_score > sem.final_score);
    }

    #[test]
    fn expected_tier_alto_com_vitoria_gera_impacto_maior() {
        let endurance = realized_with("endurance", 1, 2, true, true, false, false);
        let rookie = realized_with("mazda_rookie", 1, 2, true, true, false, false);
        assert!(endurance.final_score > rookie.final_score);
    }

    // ── Testes de repercussão — derivados ───────────────────────────────────

    #[test]
    fn media_delta_modifier_cresce_com_final_score() {
        let fraco = realized_with("mazda_rookie", 12, 12, false, false, false, false);
        let forte = realized_with("endurance", 1, 1, true, true, false, false);
        assert!(forte.media_delta_modifier > fraco.media_delta_modifier);
    }

    #[test]
    fn motivation_delta_modifier_cresce_com_final_score() {
        let fraco = realized_with("mazda_rookie", 12, 12, false, false, false, false);
        let forte = realized_with("endurance", 1, 1, true, true, false, false);
        assert!(forte.motivation_delta_modifier > fraco.motivation_delta_modifier);
    }

    #[test]
    fn headline_strength_sobe_em_grandes_eventos() {
        let pequeno = realized_with("mazda_rookie", 8, 8, false, false, false, false);
        assert_eq!(pequeno.headline_strength, HeadlineStrength::Normal);
    }

    #[test]
    fn bias_2_em_evento_principal_com_vitoria() {
        let mut ctx = base_ctx("endurance");
        ctx.season_phase = SeasonPhase::BlocoEspecial;
        ctx.is_title_decider_candidate = true;
        let expected = calculate_expected_event_interest(&ctx);
        let realized = calculate_realized_event_interest(
            &expected,
            &ctx,
            Some(1),
            Some(2),
            true,
            true,
            false,
            true,
        );
        assert_eq!(realized.news_importance_bias, 2);
        assert_eq!(realized.headline_strength, HeadlineStrength::Principal);
    }
}
